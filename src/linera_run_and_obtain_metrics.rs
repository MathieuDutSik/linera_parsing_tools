extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use chrono::{DateTime, Utc};
use common::{
    get_float, get_key_delta, get_time_string_lower, get_time_string_upper, make_file_available,
    read_config_file, read_key, read_lines_of_file,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fs::File, io::Write as _, process::Command, time::Instant};
use sysinfo::{ProcessExt, System, SystemExt};

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
    target_traces: Vec<String>,
    target_runtimes: Vec<String>,
    l_job_name: Vec<String>,
    n_iter: usize,
    kill_after_work: Vec<String>,
    file_metric_output: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SingleMetric {
    group: String,
    name: String,
    unit: String,
    values: Vec<f64>,
    counts: Vec<f64>,
}

/*
Results by the number of metrics and then by the number of runs.
 */
#[derive(Serialize, Deserialize)]
struct MultipleMetric {
    ll_metrics: Vec<Vec<SingleMetric>>,
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
                if l_str.len() < 2 {
                    println!("l_str={:?}", l_str);
                    anyhow::bail!("l_str should have length at least 2");
                }
                let key = l_str[0].to_string();
                let mut value = l_str[1].to_string();
                for i in 2..l_str.len() {
                    value += "=";
                    value += &l_str[i];
                }
                map.insert(key, value);
            }
        }
    }
    Ok(map)
}

fn get_runtime(file_name: &String, target_runtime: &String) -> f64 {
    let lines = read_lines_of_file(file_name);
    for i_line in 0..lines.len() - 2 {
        let line = &lines[i_line];
        let l_str = line
            .split(target_runtime)
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
                let value: f64 = estr.parse().unwrap();
                return value;
            }
        }
    }
    println!("ERR: file_name={file_name}");
    println!("ERR: target_runtime={target_runtime}");
    panic!("ERR: Failed to find an entry that matches ");
}

fn parse_float(line_red: &str) -> f64 {
    match line_red.parse::<f64>() {
        Err(err) => {
            println!("err={err:?}");
            println!("line_red={line_red}");
            panic!("Wrong string, please correct");
        }
        Ok(value) => value,
    }
}

fn get_millisecond(line: &str) -> f64 {
    if let Some(line_red) = line.strip_suffix("ns") {
        return parse_float(line_red) / 1000000.0;
    }
    if let Some(line_red) = line.strip_suffix("µs") {
        return parse_float(line_red) / 1000.0;
    }
    if let Some(line_red) = line.strip_suffix("ms") {
        return parse_float(line_red);
    }
    if let Some(line_red) = line.strip_suffix("s") {
        return parse_float(line_red) * 1000.0;
    }
    println!("get_millisecond, line={line}");
    panic!("Please correct");
}

fn get_busy_idle_entries(line: &str, keys: &Vec<String>) -> Option<(f64, f64)> {
    let mut main_line = line.to_string();
    for key in keys {
        let l_splt = main_line
            .split(&*key)
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        if l_splt.len() > 2 {
            println!("incorrect line={line}");
            panic!("Please correct");
        }
        if l_splt.len() == 1 {
            return None;
        }
        main_line = l_splt[1].clone();
    }
    let l_spl1 = main_line
        .split(" time.idle=")
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    if l_spl1.len() != 2 {
        return None;
    }
    let idle_val = get_millisecond(&l_spl1[1]);
    let l_spl2 = l_spl1[0]
        .split(" time.busy=")
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    if l_spl1.len() != 2 {
        return None;
    }
    let busy_val = get_millisecond(&l_spl2[1]);
    Some((busy_val, idle_val))
}

fn single_execution(iter: usize, config: &Config) -> anyhow::Result<Vec<SingleMetric>> {
    let file_out_str = format!("OUT_RUN_{}_{}.out", iter, config.n_iter);
    let file_err_str = format!("OUT_RUN_{}_{}.err", iter, config.n_iter);
    make_file_available(&file_out_str)?;
    make_file_available(&file_err_str)?;
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
    let lines = read_lines_of_file(&file_err_str);
    println!("start_time={} end_time={}", start_time, end_time);
    println!(
        "start_time_str={} end_time_str={}",
        start_time_str, end_time_str
    );
    let n_job = config.l_job_name.len();
    let mut l_metrics = Vec::new();
    //
    // The Prometheus histogram keys
    //
    for key in &config.target_prometheus_keys_hist {
        let key_sum = format!("linera_{}_sum", key);
        let key_count = format!("linera_{}_count", key);
        let data_sum = read_key(&key_sum, &config.l_job_name, &start_time_str, &end_time_str);
        let data_count = read_key(
            &key_count,
            &config.l_job_name,
            &start_time_str,
            &end_time_str,
        );
        let mut values = Vec::new();
        let mut counts = Vec::new();
        for i_job in 0..n_job {
            let len = data_sum.entries[i_job].len();
            for idx in 1..len {
                let value_upp = get_float(&data_sum.entries[i_job][idx].1);
                let value_low = get_float(&data_sum.entries[i_job][idx - 1].1);
                let count_upp = get_float(&data_count.entries[i_job][idx].1);
                let count_low = get_float(&data_count.entries[i_job][idx - 1].1);
                let value_delta = value_upp - value_low;
                let count_delta = count_upp - count_low;
                let count = count_delta as usize;
                if count > 0 {
                    let value = value_delta / count_delta;
                    values.push(value);
                    counts.push(count_delta);
                }
            }
        }
        let sm = SingleMetric {
            group: "Prometheus histogram".to_string(),
            name: key.clone(),
            unit: "ms".to_string(),
            values,
            counts,
        };
        l_metrics.push(sm);
    }
    //
    // The Prometheus fault success variables
    //
    for key_fs in &config.target_prometheus_fault_success {
        let key_f = format!("linera_{}", key_fs.fault);
        let key_s = format!("linera_{}", key_fs.success);
        let data_f = read_key(&key_f, &config.l_job_name, &start_time_str, &end_time_str);
        let data_s = read_key(&key_s, &config.l_job_name, &start_time_str, &end_time_str);
        // The length of the array data_f / data_s can be different, so only reliable way
        // is to take all entries.
        let mut values = Vec::new();
        let mut counts = Vec::new();
        for i_job in 0..n_job {
            let count_f = get_key_delta(&data_f, i_job);
            let count_s = get_key_delta(&data_s, i_job);
            if let Some(count_f) = count_f {
                if let Some(count_s) = count_s {
                    let frac = count_f / (count_f + count_s);
                    let count = count_f + count_s;
                    let count_s = count as usize;
                    if count_s > 0 {
                        let value = frac * (100 as f64);
                        values.push(value);
                        counts.push(count);
                    }
                }
            }
        }
        let sm = SingleMetric {
            group: "Prometheus fault/(fault + success)".to_string(),
            name: key_f.clone(),
            unit: "%".to_string(),
            values,
            counts,
        };
        l_metrics.push(sm);
    }
    //
    // The extraction of log metrics
    //
    for key in &config.target_log_keys {
        let mut values = Vec::new();
        let mut counts = Vec::new();
        for line in &lines {
            if line.ends_with("ms") {
                let l_str = line.split(&*key).collect::<Vec<_>>();
                if l_str.len() == 2 {
                    let sec_ent = l_str[1];
                    let sec_sel = sec_ent
                        .chars()
                        .filter(|c| c.is_numeric())
                        .collect::<String>();
                    let value = sec_sel.parse::<u64>().expect("a numerical value");
                    values.push(value as f64);
                    counts.push(1 as f64);
                }
            }
        }
        let sm = SingleMetric {
            group: "CI log".to_string(),
            name: key.clone(),
            unit: "ms".to_string(),
            values,
            counts,
        };
        l_metrics.push(sm);
    }
    //
    // The trace targets
    //
    for trace_key in &config.target_traces {
        let trace_key_busy = format!("{trace_key}_busy");
        let trace_key_idle = format!("{trace_key}_idle");
        let keys = trace_key
            .split('|')
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        let mut values_busy = Vec::new();
        let mut counts_busy = Vec::new();
        let mut values_idle = Vec::new();
        let mut counts_idle = Vec::new();
        for line in &lines {
            if let Some((busy_val, idle_val)) = get_busy_idle_entries(line, &keys) {
                values_busy.push(busy_val);
                counts_busy.push(1 as f64);
                values_idle.push(idle_val);
                counts_idle.push(1 as f64);
            }
        }
        let sm = SingleMetric {
            group: "Trace close".to_string(),
            name: trace_key_busy,
            unit: "ms".to_string(),
            values: values_busy,
            counts: counts_busy,
        };
        l_metrics.push(sm);
        let sm = SingleMetric {
            group: "Trace close".to_string(),
            name: trace_key_idle,
            unit: "ms".to_string(),
            values: values_idle,
            counts: counts_idle,
        };
        l_metrics.push(sm);
    }
    //
    // The runtime targets
    //
    for target_runtime in &config.target_runtimes {
        let value = get_runtime(&file_out_str, target_runtime);
        let values = vec![value];
        let counts = vec![1 as f64];
        let sm = SingleMetric {
            group: "runtime".to_string(),
            name: target_runtime.to_string(),
            unit: "ms".to_string(),
            values,
            counts,
        };
        l_metrics.push(sm);
    }
    //
    // The total runtime
    //
    {
        let end_time: DateTime<Utc> = Utc::now();
        let time_delta = end_time.signed_duration_since(start_time);
        let num_seconds = time_delta.num_seconds();
        println!("num_seconds={}", num_seconds);
        let num_milisecond = time_delta.num_milliseconds() as f64;
        let values = vec![num_milisecond];
        let counts = vec![1 as f64];
        let sm = SingleMetric {
            group: "runtime after".to_string(),
            name: "total runtime".to_string(),
            unit: "ms".to_string(),
            values,
            counts,
        };
        l_metrics.push(sm);
    }
    //
    // Terminating
    //
    Ok(l_metrics)
}

fn kill_after_work(config: &Config) {
    let mut system = System::new_all();

    // Refresh to get up-to-date process information
    system.refresh_all();

    // Iterate over all processes
    for (pid, process) in system.processes() {
        let mut the_name = None;
        for name in &config.kill_after_work {
            if process.name() == name {
                the_name = Some(name);
            }
        }
        if let Some(name) = the_name {
            println!(
                "Killing process: {} (PID: {pid}) name={name}",
                process.name()
            );
            // Send the `Signal::Kill` signal to the process
            if process.kill() {
                println!("Successfully killed process: {pid}: {name}");
            } else {
                eprintln!("Failed to kill process: {pid} {name}");
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
    let start_time: DateTime<Utc> = Utc::now();
    let file_input = &arguments[1];
    let config = read_config_file::<Config>(file_input)?;
    println!("commands={:?}", config.commands);
    let mut childs = Vec::new();
    for (i_command, command) in config.commands.iter().enumerate() {
        println!("i_command={i_command}");
        println!("   command={command}");
        let file_out_str = format!("OUT_COMM_{}.out", i_command);
        let file_err_str = format!("OUT_COMM_{}.err", i_command);
        make_file_available(&file_out_str)?;
        make_file_available(&file_err_str)?;
        let file_out = File::create(file_out_str)?;
        let file_err = File::create(file_err_str)?;
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
            let time_start = Instant::now();
            let output = Command::new(command)
                .stdout::<File>(file_out)
                .stderr::<File>(file_err)
                .envs(&envs)
                .args(comm_args)
                .output()?;
            println!(
                "   output={:?} in {} ms",
                output,
                time_start.elapsed().as_millis()
            );
        }
    }
    println!("------ The initial runs have been done -------");
    //
    // Running the commands iteratively
    //
    let mut ll_metrics_v1 = Vec::new();
    for iter in 0..config.n_iter {
        println!(
            "--------------------- {}/{} --------------------",
            iter, config.n_iter
        );
        let l_metrics = single_execution(iter, &config)?;
        let mut missing_keys = Vec::new();
        for rec in &l_metrics {
            if rec.values.len() == 0 {
                missing_keys.push(rec.name.clone());
            }
        }
        println!("missing_keys={missing_keys:?}");
        ll_metrics_v1.push(l_metrics);
    }
    //
    // Transposing the matrix and keeping the non-zero entries
    //
    println!("-------------- Transposing the matrix ---------------");
    let n_keys = ll_metrics_v1[0].len();
    let mut ll_metrics_v2 = Vec::new();
    for _i_key in 0..n_keys {
        ll_metrics_v2.push(Vec::new());
    }
    for var_r in ll_metrics_v1 {
        for i_key in 0..n_keys {
            ll_metrics_v2[i_key].push(var_r[i_key].clone());
        }
    }
    let mut ll_metrics_v3 = Vec::new();
    for l_metrics in ll_metrics_v2 {
        let mut tot_len = 0;
        for metrics in &l_metrics {
            tot_len += metrics.values.len();
        }
        if tot_len > 0 {
            ll_metrics_v3.push(l_metrics);
        }
    }
    //
    // Now saving the data
    //
    println!("Data has been computed");
    let file_metric_output = config.file_metric_output.clone();
    println!("file_metric_output={file_metric_output}");
    let mm = MultipleMetric {
        ll_metrics: ll_metrics_v3,
    };
    let json_string = serde_json::to_string(&mm)?;
    let mut file = File::create(file_metric_output)?;
    file.write_all(json_string.as_bytes())?;
    println!("Data has been written");
    //
    // Kill processes
    //
    kill_after_work(&config);
    let end_time: DateTime<Utc> = Utc::now();
    let time_delta = end_time.signed_duration_since(start_time);
    let num_seconds = time_delta.num_seconds();
    println!("num_seconds={num_seconds}");
    Ok(())
}
