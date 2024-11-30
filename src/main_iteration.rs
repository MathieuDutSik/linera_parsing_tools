extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::File;
use std::process::Command;
use sysinfo::{ProcessExt, System, SystemExt};
use std::io::Write as _;

use common::{get_float, get_time_string_lower, get_time_string_upper, read_config_file, read_key, read_lines_of_file};

#[derive(Deserialize)]
struct SingleEnvironmentList {
    command: String,
    environments: Vec<String>,
}

#[derive(Deserialize)]
struct SingleFaultSuccess {
    fault: String,
    success: String,
}

#[derive(Deserialize)]
struct Config {
    environments: Vec<SingleEnvironmentList>,
    commands: Vec<String>,
    critical_command: String,
    target_prometheus_keys_hist: Vec<String>,
    target_log_keys: Vec<String>,
    target_prometheus_fault_success: Vec<SingleFaultSuccess>,
    runtime_targets: Vec<String>,
    l_job_name: Vec<String>,
    n_iter: usize,
    skip: usize,
    kill_after_work: Vec<String>,
    print_all_vals: bool,
    file_metric_output: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SingleMetric {
    group: String,
    name: String,
    value: f64,
    unit: String,
}


#[derive(Serialize, Deserialize)]
struct MultipleMetric {
    metrics_result: Vec<SingleMetric>,
}

/*
Results from the run, the entries are by the job_name, and then by the variable name.
*/
#[derive(Debug)]
struct ResultSingleRun {
    prometheus_hist: Vec<Vec<Option<f64>>>,
    prometheus_fault_success: Vec<Vec<Option<f64>>>,
    log_key_metrics: Vec<Option<f64>>,
    runtimes: Vec<f64>,
}

fn get_environments(config: &Config, command: &String) -> anyhow::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let start_str = "export ";
    for sel in &config.environments {
        if &sel.command == command {
            for entry in &sel.environments {
                if !entry.starts_with(start_str) {
                    anyhow::bail!("Should starts with export ");
                }
                let entry = &entry[start_str.len()..];
                let l_str = entry.split('=').map(|x| x.to_string()).collect::<Vec<_>>();
                if l_str.len() != 2 {
                    println!("l_str={:?}", l_str);
                    anyhow::bail!("l_str should have length 2");
                }
                let key = l_str[0].to_string();
                let value = l_str[1].to_string();
                map.insert(key, value);
            }
        }
    }
    Ok(map)
}

fn get_runtime(file_name: &String, runtime_target: &String) -> f64 {
    let lines = read_lines_of_file(file_name);
    for i_line in 0..lines.len() - 2 {
        let line = &lines[i_line];
        let l_str = line
            .split(runtime_target)
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        if l_str.len() == 2 {
            let line = &lines[i_line + 2];
            let l_str = line
                .split("finished in ")
                .map(|x| x.to_string())
                .collect::<Vec<_>>();
            if l_str.len() == 2 {
                let estr = &l_str[1];
                let estr = &estr[..estr.len() - 1];
                let val: f64 = estr.parse().unwrap();
                return val;
            }
        }
    }
    panic!("Failed to find an entry that matches");
}

fn single_execution(iter: usize, config: &Config) -> anyhow::Result<ResultSingleRun> {
    let file_out_str = format!("OUT_RUN_{}_{}.out", iter, config.n_iter);
    let file_err_str = format!("OUT_RUN_{}_{}.err", iter, config.n_iter);
    println!(
        "single_execution file_out_str={} file_err_str={}",
        file_out_str, file_err_str
    );
    let file_out = File::create(file_out_str.clone())?;
    let file_err = File::create(file_err_str.clone())?;
    let start_time: DateTime<Utc> = Utc::now();
    let envs = get_environments(config, &config.critical_command)?;
    println!("single_execution envs={:?}", envs);
    let l_str = config
        .critical_command
        .split(' ')
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    let command = &l_str[0];
    println!("single_execution command={}", command);
    let mut comm_args = Vec::new();
    for i in 1..l_str.len() {
        comm_args.push(l_str[i].clone());
    }
    println!("single_execution comm_args={:?}", comm_args);
    let _output = Command::new(command)
        .stdout::<File>(file_out)
        .stderr::<File>(file_err)
        .envs(&envs)
        .args(comm_args)
        .output()?;
    let end_time: DateTime<Utc> = Utc::now();
    let start_time_str = get_time_string_lower(start_time);
    let end_time_str = get_time_string_upper(end_time);
    println!("start_time={} end_time={}", start_time, end_time);
    println!(
        "start_time_str={} end_time_str={}",
        start_time_str, end_time_str
    );
    //
    // The Prometheus keys
    //
    let mut prometheus_hist: Vec<Vec<Option<f64>>> = Vec::new();
    let n_job = config.l_job_name.len();
    let n_keys = config.target_prometheus_keys_hist.len();
    for _i_job in 0..n_job {
        let mut v = Vec::new();
        for _i_key in 0..n_keys {
            v.push(None);
        }
        prometheus_hist.push(v);
    }
    for i_key in 0..n_keys {
        let key = &config.target_prometheus_keys_hist[i_key];
        let key_sum = format!("linera_{}_sum", key);
        let key_count = format!("linera_{}_count", key);
        let data_sum = read_key(&key_sum, &config.l_job_name, &start_time_str, &end_time_str);
        let data_count = read_key(
            &key_count,
            &config.l_job_name,
            &start_time_str,
            &end_time_str,
        );
        for i_job in 0..n_job {
            let len = data_count.entries[i_job].len();
            if len > 0 {
                let count_tot = get_float(&data_count.entries[i_job][len - 1].1);
                let value_tot = get_float(&data_sum.entries[i_job][len - 1].1);
                let avg = value_tot / count_tot;
                prometheus_hist[i_job][i_key] = Some(avg);
            }
        }
    }
    //
    // The Prometheus fault success variables
    //
    let mut prometheus_fault_success: Vec<Vec<Option<f64>>> = Vec::new();
    let n_fs = config.target_prometheus_fault_success.len();
    for _i_job in 0..n_job {
        let mut v = Vec::new();
        for _i_fs in 0..n_fs {
            v.push(None);
        }
        prometheus_fault_success.push(v);
    }
    for i_fs in 0..n_fs {
        let key_f = format!("linera_{}", config.target_prometheus_fault_success[i_fs].fault);
        let key_s = format!("linera_{}", config.target_prometheus_fault_success[i_fs].success);
        let data_f = read_key(&key_f, &config.l_job_name, &start_time_str, &end_time_str);
        let data_s = read_key(&key_s, &config.l_job_name, &start_time_str, &end_time_str);
        for i_job in 0..n_job {
            let len_s = data_s.entries[i_job].len();
            let len_f = data_s.entries[i_job].len();
            if len_s > 0 && len_f > 0 {
                let count_f = get_float(&data_f.entries[i_job][len_f - 1].1);
                let count_s = get_float(&data_s.entries[i_job][len_s - 1].1);
                let perf = count_s / (count_f + count_s);
                prometheus_fault_success[i_job][i_fs] = Some(perf);
            }
        }
    }
    //
    // The extraction of log metrics
    //
    let mut log_key_metrics: Vec<Option<f64>> = Vec::new();
    let n_log_keys = config.target_log_keys.len();
    let lines = read_lines_of_file(&file_err_str);
    for i_log_key in 0..n_log_keys {
        let key = &config.target_log_keys[i_log_key];
        let mut n_ms = 0 as f64;
        let mut count = 0;
        for line in &lines {
            if line.ends_with("ms") {
                let l_str = line.split(&*key).collect::<Vec<_>>();
                if l_str.len() == 2 {
                    let sec_ent = l_str[1];
                    let sec_sel = sec_ent.chars()
                        .filter(|c| c.is_numeric())
                        .collect::<String>();
                    let value = sec_sel.parse::<u64>().expect("a numerical value");
                    n_ms += value as f64;
                    count += 1;
                }
            }
        }
        let key_metric = if count > 0 {
            let avg = n_ms / (count as f64);
            Some(avg)
        } else {
            None
        };
        log_key_metrics.push(key_metric);
    }
    //
    // The runtime targets
    //
    let mut runtimes = Vec::new();
    for runtime_target in &config.runtime_targets {
        let runtime = get_runtime(&file_out_str, runtime_target);
        runtimes.push(runtime);
    }
    Ok(ResultSingleRun {
        prometheus_hist,
        prometheus_fault_success,
        log_key_metrics,
        runtimes,
    })
}

fn kill_after_work(config: &Config) {
    let mut system = System::new_all();

    // Refresh to get up-to-date process information
    system.refresh_all();

    // Iterate over all processes
    for (pid, process) in system.processes() {
        let mut is_matching = false;
        for name in &config.kill_after_work {
            if process.name() == name {
                is_matching = true;
            }
        }
        if is_matching {
            println!("Killing process: {} (PID: {})", process.name(), pid);
            // Send the `Signal::Kill` signal to the process
            if process.kill() {
                println!("Successfully killed process: {}", pid);
            } else {
                eprintln!("Failed to kill process: {}", pid);
            }
        }
    }
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
        println!("running commands [FileI]");
        std::process::exit(1)
    }
    let file_input = &arguments[1];
    let config = read_config_file::<Config>(file_input)?;
    println!("commands={:?}", config.commands);
    let mut childs = Vec::new();
    for (i_command, command) in config.commands.iter().enumerate() {
        println!("i_command={} command={}", i_command, command);
        let file_out = format!("OUT_COMM_{}.out", i_command);
        let file_out = File::create(file_out)?;
        let file_err = format!("OUT_COMM_{}.err", i_command);
        let file_err = File::create(file_err)?;
        let len_command = command.len();
        if command.ends_with(" &") {
            let red_command = command[..len_command - 2].to_string();
            println!("   red_command={}", red_command);
            let envs = get_environments(&config, &red_command)?;
            println!("   SPA envs={:?}", envs);
            let l_str = red_command
                .split(' ')
                .map(|x| x.to_string())
                .collect::<Vec<_>>();
            let command = &l_str[0];
            let mut comm_args = Vec::new();
            for i in 1..l_str.len() {
                comm_args.push(l_str[i].clone());
            }
            let child = Command::new(command)
                .stdout::<File>(file_out)
                .stderr::<File>(file_err)
                .envs(&envs)
                .args(comm_args)
                .spawn()?;
            childs.push(child);
        } else {
            let l_str = command
                .split(' ')
                .map(|x| x.to_string())
                .collect::<Vec<_>>();
            let command = &l_str[0];
            let envs = get_environments(&config, command)?;
            println!("   DIR envs={:?}", envs);
            let mut comm_args = Vec::new();
            for i in 1..l_str.len() {
                comm_args.push(l_str[i].clone());
            }
            let output = Command::new(command)
                .stdout::<File>(file_out)
                .stderr::<File>(file_err)
                .envs(&envs)
                .args(comm_args)
                .output()?;
            println!("output={:?}", output);
        }
    }
    println!("------ The initial runs have been done -------");
    let mut metrics_result = Vec::new();
    //
    // Running the commands iteratively
    //
    let mut var_results: Vec<ResultSingleRun> = Vec::new();
    for iter in 0..config.n_iter {
        let var_result = single_execution(iter, &config)?;
        println!("var_result={:?}", var_result);
        var_results.push(var_result);
    }
    //
    // Printing the prometheus keys of histogram
    //
    println!("-------------- Prometheus Keys histograms ---------------");
    let n_key = config.target_prometheus_keys_hist.len();
    let n_job = config.l_job_name.len();
    for i_job in 0..n_job {
        println!("Metrics for job={}", config.l_job_name[i_job]);
        for i_key in 0..n_key {
            let mut n_miss = 0;
            let mut sum_val = 0 as f64;
            let mut count = 0;
            let mut vals = Vec::new();
            for iter in config.skip..config.n_iter {
                match var_results[iter].prometheus_hist[i_job][i_key] {
                    None => {
                        n_miss += 1;
                    }
                    Some(val) => {
                        sum_val += val;
                        count += 1;
                        vals.push(val);
                    }
                }
            }
            let key = &config.target_prometheus_keys_hist[i_key];
            if count > 0 {
                let avg = sum_val / (count as f64);
                print!("  key={} n_miss={} avg={}", key, n_miss, avg);
                if config.print_all_vals {
                    print!(" vals={:?}", vals);
                }
                println!();
                let sm = SingleMetric {
                    group: "prometheus_hist".to_string(),
                    name: key.clone(),
                    value: avg,
                    unit: "ms".to_string(),
                };
                metrics_result.push(sm);
            } else {
                println!("  No metric for {}", key);
            }
        }
    }
    //
    // Printing the fault/success statistics
    //
    println!("--------------- Prometheus fault/success statistics -------------");
    let n_fs = config.target_prometheus_fault_success.len();
    for i_job in 0..n_job {
        println!(
            "Fault/Success metyrics for job={}",
            config.l_job_name[i_job]
        );
        for i_fs in 0..n_fs {
            let mut n_miss = 0;
            let mut sum_val = 0 as f64;
            let mut count = 0 as f64;
            let mut vals = Vec::new();
            for iter in config.skip..config.n_iter {
                match var_results[iter].prometheus_fault_success[i_job][i_fs] {
                    None => {
                        n_miss += 1;
                    }
                    Some(val) => {
                        sum_val += val;
                        count += 1.0;
                        vals.push(val);
                    }
                }
            }
            let key_f = &config.target_prometheus_fault_success[i_fs].fault;
            let key_s = &config.target_prometheus_fault_success[i_fs].success;
            if vals.len() > 0 {
                let avg = sum_val / count;
                print!(
                    "  key={} / {} n_miss={} avg={}",
                    key_f, key_s, n_miss, avg
                );
                if config.print_all_vals {
                    print!(" vals={:?}", vals);
                }
                println!();
                let sm = SingleMetric {
                    group: "prometheus_fault_success".to_string(),
                    name: key_f.clone(),
                    value: avg,
                    unit: "%".to_string(),
                };
                metrics_result.push(sm);
            } else {
                println!("  No metric for {} / {}", key_f, key_s);
            }
        }
    }
    //
    // Printing the log metrics
    //
    println!("---------------- Log key metrics ----------------");
    let n_log_keys = config.target_log_keys.len();
    for i_log_key in 0..n_log_keys {
        let key = config.target_log_keys[i_log_key].clone();
        let mut sum_val = 0 as f64;
        let mut count = 0;
        let mut n_miss = 0;
        let mut vals = Vec::new();
        for iter in config.skip..config.n_iter {
            match var_results[iter].log_key_metrics[i_log_key] {
                None => {
                    n_miss += 1;
                }
                Some(val) => {
                    sum_val += val;
                    count += 1;
                    vals.push(val);
                }
            }
        }
        if count > 0 {
            let avg = sum_val / (count as f64);
            print!("key={} avg={}", key, avg);
            if config.print_all_vals {
                print!(" vals={:?}", vals);
            }
            println!();
            let sm = SingleMetric {
                group: "ci_log".to_string(),
                name: key.clone(),
                value: avg,
                unit: "ms".to_string(),
            };
            metrics_result.push(sm);
        } else {
            println!("The key={} did not match anything in the log n_miss={}", key, n_miss);
        }
    }
    //
    // Printing the total runtime
    //
    println!("------------- The total runtime of the test ------------");
    let n_rt = config.runtime_targets.len();
    for i_rt in 0..n_rt {
        let mut sum_val = 0 as f64;
        let mut count = 0 as f64;
        let mut vals = Vec::new();
        for iter in config.skip..config.n_iter {
            let val = var_results[iter].runtimes[i_rt];
            sum_val += val;
            count += 1.0;
            vals.push(val);
        }
        let avg = sum_val / count;
        print!("runtime, avg={}", avg);
        if config.print_all_vals {
            print!(" vals={:?}", vals)
        }
        println!();
        let sm = SingleMetric {
            group: "runtime_target".to_string(),
            name: config.runtime_targets[i_rt].clone(),
            value: avg,
            unit: "ms".to_string(),
        };
        metrics_result.push(sm);
    }
    //
    // Now saving the data
    //
    if let Some(file_metric_output) = config.file_metric_output.clone() {
        let mm = MultipleMetric { metrics_result };
        let json_string = serde_json::to_string(&mm)?;
        let mut file = File::open(file_metric_output)?;
        file.write_all(json_string.as_bytes())?;
    }
    kill_after_work(&config);
    Ok(())
}
