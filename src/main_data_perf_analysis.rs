extern crate chrono;
extern crate serde_json;
mod common;
use serde::{Serialize, Deserialize};
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

#[derive(Serialize, Deserialize)]
struct SingleMetric {
    group: String,
    name: String,
    unit: String,
    values: Vec<f64>,
    counts: Vec<f64>,
}

fn compute_average(values: Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_average");
    }
    let mut sum = 0 as f64;
    for value in values {
        sum += value;
    }
    let avg = sum / (len as f64);
    avg
}

fn compute_stddev(values: Vec<f64>) -> f64 {
    let len = values.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_average");
    }
    let mut sum_p1 = 0 as f64;
    let mut sum_p2 = 0 as f64;
    for value in values {
        sum_p1 += value;
        sum_p2 += value * value;
    }
    let avg_p1 = sum_p1 / (len as f64);
    let avg_p2 = sum_p2 / (len as f64);
    let variance = avg_p2 - avg_p1 * avg_p1;
    let stddev = variance.sqrt();
    stddev
}

fn compute_median(mut data: Vec<f64>) -> f64 {
    let len = data.len();
    if len == 0 {
        panic!("We should have a non-zero number of values in compute_median");
    }

    data.sort_by(|a, b| a.partial_cmp(b).unwrap()); // Handle sorting
    let mid = len / 2;

    if len % 2 == 0 {
        // Average of two middle values
        (data[mid - 1] + data[mid]) / 2.0
    } else {
        // Middle value
        data[mid]
    }
}

fn compute_mean(values: Vec<f64>, method: &str) -> f64 {
    if method == "average" {
        return compute_average(values);
    }
    if method == "stddev" {
        return compute_stddev(values);
    }
    if method == "median" {
        return compute_median(values);
    }
    panic!("method={method} but allowed methods are average / median");
}

fn data_dropping(values: Vec<f64>, method: &str) -> Vec<f64> {
    if method == "half" {
        let len = values.len();
        let mid = len / 2;
        return values[mid..].to_vec();
    }
    if let Some(remain) = method.strip_prefix("skip") {
        let n_skip : usize = remain.parse::<usize>().expect("An integer");
        return values[n_skip..].to_vec();
    }
    panic!("Unsopported data droppping method");
}

#[derive(Serialize, Deserialize)]
struct MultipleMetric {
    metrics_result: Vec<SingleMetric>,
}

fn get_entry(value: f64, unit: &str) -> String {
    if value > 1000.0 && unit == "ms" {
        let value_red = value / (1000 as f64);
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
    let args = std::env::args();
    let mut arguments = Vec::new();
    for argument in args {
        println!("argument={argument}");
        arguments.push(argument);
    }
    let n_arg = arguments.len();
    println!("n_arg={}", n_arg);
    if n_arg == 1 {
        println!("Program is used as");
        println!("running slack_benchmarks_formatting [FileI]");
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
        for entry in &result.metrics_result {
            let key : (String, String) = (entry.group.clone(), entry.name.clone());
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
            if l_set[i_run].get(&key).is_none() {
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
        let mut metrics_result = Vec::new();
        for sm in metrics.metrics_result {
            let key : (String, String) = (sm.group.clone(), sm.name.clone());
            if set_int.get(&key).is_some() {
                metrics_result.push(sm);
            } else {
                println!("Dropping {} : {}", sm.group, sm.name);
            }
        }
        let mm = MultipleMetric { metrics_result };
        l_metrics_red.push(mm);
    }
    //
    // The method chosen
    //
    let data_dropping_strategy = config.data_dropping_strategy.clone();
    let mean_strategy = config.mean_strategy.clone();
    let print_all_vals = config.print_all_vals;
    let bold_string = get_bold(&config.choice_format);
    //
    // The output
    //
    let mut current_group = "unset".to_string();
    for i_metric in 0..n_metric {
        let group = l_metrics_red[0].metrics_result[i_metric].group.clone();
        let metric_name = l_metrics_red[0].metrics_result[i_metric].name.clone();
        let unit = l_metrics_red[0].metrics_result[i_metric].unit.clone();
        if group != current_group {
            if i_metric != 0 {
                println!();
            }
            current_group = group.clone();
            println!("{group}:");
        }
        let mut idx_best = 0;
        let mut best_metric = 0 as f64;
        let mut all_counts = Vec::new();
        let mut metrics = Vec::new();
        for i_run in 0..n_runs {
            let values = l_metrics_red[i_run].metrics_result[i_metric].values.clone();
            let values = data_dropping(values, &data_dropping_strategy);
            let metric = compute_mean(values, &mean_strategy);
            metrics.push(metric);
            //
            let counts = l_metrics_red[i_run].metrics_result[i_metric].counts.clone();
            let counts = data_dropping(counts, &data_dropping_strategy);
            all_counts.extend(counts);
            //
            let group_b = l_metrics_red[i_run].metrics_result[i_metric].group.clone();
            let metric_name_b = l_metrics_red[i_run].metrics_result[i_metric].name.clone();
            if group != group_b || metric_name != metric_name_b {
                panic!("The ordering of the entries in the file is not the same");
            }
            if i_run == 0 {
                idx_best = i_run;
                best_metric = metric;
            } else {
                if metric < best_metric {
                    idx_best = i_run;
                    best_metric = metric;
                }
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
            for i_run in 0..n_runs {
                let name = config.names[i_run].clone();
                println!("{name} : vals={:?}", l_metrics_red[i_run].metrics_result[i_metric].values);
            }
        }
    }
    Ok(())
}
