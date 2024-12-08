extern crate chrono;
extern crate serde_json;
mod common;
use serde::{Serialize, Deserialize};

use common::{nice_float_str, read_config_file};

#[derive(Deserialize)]
struct Config {
    names: Vec<String>,
    log_files: Vec<String>,
    choice_format: String,
}

#[derive(Serialize, Deserialize)]
struct SingleMetric {
    group: String,
    name: String,
    unit: String,
    value: f64,
    count: f64,
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

    let mut l_metrics = Vec::new();
    let mut n_metric = 0;
    for log_file in &config.log_files {
        let result = read_config_file::<MultipleMetric>(log_file)?;
        let len_metric = result.metrics_result.len();
        l_metrics.push(result);
        if n_metric == 0 {
            n_metric = len_metric;
        } else {
            if n_metric != len_metric {
                println!("n_metric={n_metric} len_metric={len_metric} so it is inconsistent and we cannot run formard");
                panic!("Please correct the input");
            }
        }
    }
    let n_runs = config.log_files.len();
    let bold_string = get_bold(&config.choice_format);
    //
    // The output
    //
    let mut current_group = "unset".to_string();
    for i_metric in 0..n_metric {
        let group = l_metrics[0].metrics_result[i_metric].group.clone();
        let metric_name = l_metrics[0].metrics_result[i_metric].name.clone();
        let unit = l_metrics[0].metrics_result[i_metric].unit.clone();
        if group != current_group {
            if i_metric != 0 {
                println!();
            }
            current_group = group.clone();
            println!("{group}:");
        }
        let mut idx_best = 0;
        let mut best_metric = 0 as f64;
        for i_run in 0..n_runs {
            let metric = l_metrics[i_run].metrics_result[i_metric].value;
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
        let mut sum_count = 0 as f64;
        for i_run in 0..n_runs {
            let count = l_metrics[i_run].metrics_result[i_metric].count;
            sum_count += count;
        }
        let avg_count = sum_count / (n_runs as f64);
        for i_run in 0..n_runs {
            let name = config.names[i_run].clone();
            let metric = l_metrics[i_run].metrics_result[i_metric].value;
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
        print!(": {metric_name_red} ({avg_count} times)");
        println!();
    }
    Ok(())
}
