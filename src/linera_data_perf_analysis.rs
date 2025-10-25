extern crate chrono;
extern crate serde_json;
mod common;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use common::{nice_float_str, read_config_file};

#[derive(Deserialize)]
struct Config {
    names: Vec<String>,
    log_files: Vec<String>,
    data_dropping_strategy: String,
    mean_strategy: String,
    print_all_vals: bool,
    choice_format: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SingleMetric {
    group: String,
    name: String,
    unit: String,
    values: Vec<f64>,
    counts: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
struct MultipleMetric {
    ll_metrics: Vec<Vec<SingleMetric>>,
}

fn compute_average(values: Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_average");
    }
    let mut sum = 0_f64;
    for value in values {
        sum += value;
    }
    sum / (len as f64)
}

fn compute_lowest(values: &Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_lowest");
    }
    let mut min_val = values[0];
    for val in values {
        if *val < min_val {
            min_val = *val;
        }
    }
    min_val
}

fn compute_highest(values: &Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_lowest");
    }
    let mut max_val = values[0];
    for val in values {
        if *val > max_val {
            max_val = *val;
        }
    }
    max_val
}

fn compute_weighted_average(values: &[f64], counts: &Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_weighted_average");
    }
    let mut sum_val = 0_f64;
    let mut sum_cnt = 0_f64;
    for (value, count) in values.iter().zip(counts) {
        sum_val += count * value;
        sum_cnt += count;
    }
    sum_val / sum_cnt
}

fn compute_sum_runtimes(values: &[f64], counts: &Vec<f64>) -> f64 {
    let mut sum_val = 0_f64;
    for (value, count) in values.iter().zip(counts) {
        sum_val += count * value;
    }
    sum_val
}

fn compute_weighted_stddev(values: &[f64], counts: &Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_weighted_stddev");
    }
    let mut sum_p0 = 0_f64;
    let mut sum_p1 = 0_f64;
    let mut sum_p2 = 0_f64;
    for (value, count) in values.iter().zip(counts) {
        sum_p0 += count;
        sum_p1 += count * value;
        sum_p2 += count * value * value;
    }
    let avg_p1 = sum_p1 / sum_p0;
    let avg_p2 = sum_p2 / sum_p0;
    let variance = avg_p2 - avg_p1 * avg_p1;
    variance.sqrt()
}

fn compute_weighted_median(values: &[f64], counts: &Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_weighted_median");
    }
    let mut data = Vec::new();
    let mut sum_weight = 0_f64;
    for (value, count) in values.iter().zip(counts) {
        sum_weight += count;
        data.push((value, count));
    }
    data.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap()); // Handle sorting
    let target_weight = sum_weight / 2.0;
    let mut pos = 0_f64;
    for idx in 0..data.len() {
        pos += data[idx].1;
        if pos > target_weight {
            return *data[idx].0;
        }
    }
    panic!("Failed to find index");
}

fn compute_weighted_mean(values: &Vec<f64>, counts: &Vec<f64>, method: &str) -> f64 {
    if method == "average" {
        return compute_weighted_average(values, counts);
    }
    if method == "stddev" {
        return compute_weighted_stddev(values, counts);
    }
    if method == "median" {
        return compute_weighted_median(values, counts);
    }
    if method == "lowest" {
        return compute_lowest(values);
    }
    if method == "highest" {
        return compute_highest(values);
    }
    if method == "sum_runtimes" {
        return compute_sum_runtimes(values, counts);
    }
    panic!("method={method} but allowed methods are average / stddev / median");
}

fn first_used_index(n_iter: usize, method: &str) -> usize {
    if method == "half" {
        let mid = n_iter / 2;
        return mid;
    }
    if let Some(remain) = method.strip_prefix("skip") {
        let n_skip: usize = remain.parse::<usize>().expect("An integer");
        return n_skip;
    }
    panic!("Unsupported data droppping method");
}

fn get_entry(value: f64, unit: &str) -> String {
    if value > 1000.0 && unit == "ms" {
        let value_red = value / (1000_f64);
        format!("{} s", nice_float_str(value_red))
    } else {
        format!("{} {}", nice_float_str(value), unit)
    }
}

fn get_bold(choice_format: &str) -> String {
    if choice_format == "GitHub" {
        return "**".to_string();
    }
    if choice_format == "Slack" {
        return "*".to_string();
    }
    panic!("choice_format can be GitHub or Slack");
}

fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    let n_arg = arguments.len();
    if n_arg != 2 {
        println!("Program is used as");
        println!("linera_data_perf_analysis [FileI]");
        std::process::exit(1)
    }
    let file_input = &arguments[1];
    let config = read_config_file::<Config>(file_input)?;
    let n_runs = config.log_files.len();
    //
    // Reading the metrics and identifiying the common ones.
    //
    let mut l_metrics = Vec::new();
    let mut l_set = Vec::new();
    for log_file in &config.log_files {
        let result = read_config_file::<MultipleMetric>(log_file)?;
        let mut set = BTreeSet::new();
        for entry in &result.ll_metrics {
            let key: (String, String) = (entry[0].group.clone(), entry[0].name.clone());
            set.insert(key);
        }
        println!("log_file={log_file} |set|={}", set.len());
        l_set.push(set);
        l_metrics.push(result);
    }
    let mut set_int = BTreeSet::new();
    for key in l_set[0].clone() {
        let mut is_present = true;
        for i_run in 1..n_runs {
            if l_set[i_run].contains(&key) {
                is_present = false;
            }
        }
        if is_present {
            set_int.insert(key);
        }
    }
    println!("|set_int|={}", set_int.len());
    let n_metric = set_int.len();
    let mut l_metrics_red = Vec::new();
    for metrics in l_metrics {
        let mut ll_metrics = Vec::new();
        for sm in metrics.ll_metrics {
            let key: (String, String) = (sm[0].group.clone(), sm[0].name.clone());
            if set_int.contains(&key) {
                ll_metrics.push(sm);
            } else {
                println!("Dropping {} : {}", key.0, key.1);
            }
        }
        let mm = MultipleMetric { ll_metrics };
        l_metrics_red.push(mm);
    }
    //
    // The method chosen
    //
    let data_dropping_strategy = config.data_dropping_strategy.clone();
    let mean_strategy = config.mean_strategy.clone();
    let print_all_vals = config.print_all_vals;
    let bold_string = get_bold(&config.choice_format);
    let n_iter = l_metrics_red[0].ll_metrics[0].len();
    let first_index = first_used_index(n_iter, &data_dropping_strategy);
    //
    // The output
    //
    let mut current_group = "unset".to_string();
    for i_metric in 0..n_metric {
        let group = l_metrics_red[0].ll_metrics[i_metric][0].group.clone();
        let metric_name = l_metrics_red[0].ll_metrics[i_metric][0].name.clone();
        let unit = l_metrics_red[0].ll_metrics[i_metric][0].unit.clone();
        if group != current_group {
            if i_metric != 0 {
                println!();
            }
            current_group = group.clone();
            println!("{group}:");
        }
        let mut idx_best = 0;
        let mut best_metric = 0_f64;
        let mut all_counts = Vec::new();
        let mut metrics = Vec::new();
        let mut l_values = Vec::new();
        for i_run in 0..n_runs {
            let v = l_metrics_red[i_run].ll_metrics[i_metric].clone();
            let group_b = v[0].group.clone();
            let metric_name_b = v[0].name.clone();
            if group != group_b || metric_name != metric_name_b {
                panic!("The ordering of the entries in the file is not the same");
            }
            let mut values = Vec::new();
            let mut counts = Vec::new();
            for vec in &v[first_index..] {
                values.extend(vec.values.clone());
                let mut sum_count = 0_f64;
                for ent in &vec.counts {
                    sum_count += ent;
                }
                all_counts.push(sum_count);
                counts.extend(vec.counts.clone());
            }
            let metric = compute_weighted_mean(&values, &counts, &mean_strategy);
            l_values.push(values);
            metrics.push(metric);
            //
            if i_run == 0 {
                idx_best = i_run;
                best_metric = metric;
            } else if metric < best_metric {
                idx_best = i_run;
                best_metric = metric;
            }
        }
        print!("* ");
        let avg_count = compute_average(all_counts);
        for i_run in 0..n_runs {
            let name = config.names[i_run].clone();
            let metric = metrics[i_run];
            if i_run > 0 {
                print!(", ");
            }
            let str_out = get_entry(metric, &unit);
            if i_run == idx_best {
                print!("{bold_string}{str_out}{bold_string}({name})")
            } else {
                print!("{str_out}({name})")
            }
        }
        let metric_name_red = metric_name.replace("_", " ");
        print!(": {metric_name_red} ({avg_count:.2} times)");
        println!();
        if print_all_vals {
            for (i_run, values) in l_values.into_iter().enumerate() {
                let name = config.names[i_run].clone();
                println!("{name} : vals={:?}", values);
            }
        }
    }
    Ok(())
}
