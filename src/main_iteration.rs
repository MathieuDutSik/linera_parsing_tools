extern crate serde;
extern crate serde_json;
extern crate chrono;
mod common;
use serde::{Deserialize};
use std::fs::File;
use std::io::BufReader;
use std::process::{
    Command,
};
use chrono::{Utc, DateTime};

use common::{get_float, get_request_string, read_key, read_linera_keys};



#[derive(Deserialize)]
struct Config {
    commands: Vec<String>,
    critical_command: String,
    target_keys_hist: Vec<String>,
    runtime_target: String,
    l_job_name: Vec<String>,
    n_iter: usize,
}

/*
Results from the run, the entries are by the job_name, and then by the variable name.
*/
struct ResultSingleRun {
    results: Vec<Vec<Option<f64>>>,
    runtime: f64,
}





fn execute_and_estimate_runtime(iter: usize, config: &Config) -> anyhow::Result<ResultSingleRun> {
    let file_out = format!("OUT_RUN_{}_{}.out", iter, config.n_iter);
    let file_out = File::create(file_out)?;
    let file_err = format!("OUT_RUN_{}_{}.err", iter, config.n_iter);
    let file_err = File::create(file_err)?;
    let start_time: DateTime<Utc> = Utc::now();
    let l_str = config.runtime_target.split(' ').map(|x| x.to_string()).collect::<Vec<_>>();
    let command = &l_str[0];
    let mut comm_args = Vec::new();
    for i in 1..l_str.len() {
        comm_args.push(l_str[i].clone());
    }
    let _output = Command::new(command)
        .stdout::<File>(file_out)
        .stderr::<File>(file_err)
        .args(comm_args)
        .output()?;
    let end_time: DateTime<Utc> = Utc::now();
    let start_time_str = start_time.to_string();
    let end_time_str = end_time.to_string();
    let mut results : Vec<Vec<Option<f64>>> = Vec::new();
    let n_job = config.l_job_name.len();
    let n_keys = config.target_keys_hist.len();
    for _i_job in 0..n_job {
        let mut v = Vec::new();
        for _i_key in 0..n_keys {
            v.push(None);
        }
        results.push(v);
    }
    for i_key in 0..n_keys {
        let key = &config.target_keys_hist[i_key];
        let key_sum = format!("linera_{}_sum", key);
        let key_count = format!("linera_{}_count", key);
        let data_sum = read_key(&key_sum, &config.l_job_name, &start_time_str, &end_time_str);
        let data_count = read_key(&key_count, &config.l_job_name, &start_time_str, &end_time_str);
        for i_job in 0..n_job {
            let len = data_count.entries[i_job].len();
            if len > 0 {
                let count_tot = get_float(&data_count.entries[i_job][len - 1].1);
                let value_tot = get_float(&data_sum.entries[i_job][len - 1].1);
                let avg = value_tot / count_tot;
                results[i_job][i_key] = Some(avg);
            }
        }
    }
    let runtime = 0 as f64;
    Ok(ResultSingleRun { results, runtime })
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
    let file = File::open(file_input)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)?;
    println!("commands={:?}", config.commands);
    let mut childs = Vec::new();
    let mut i_command = 0;
    for command in &config.commands {
        let file_out = format!("OUT_COMM_{}.out", i_command);
        let file_out = File::create(file_out)?;
        let file_err = format!("OUT_COMM_{}.err", i_command);
        let file_err = File::create(file_err)?;
        let len_command = command.len();
        if command.ends_with(" &") {
            let red_command = command[..len_command-2].to_string();
            let l_str = red_command.split(' ').map(|x| x.to_string()).collect::<Vec<_>>();
            let command = &l_str[0];
            let mut comm_args = Vec::new();
            for i in 1..l_str.len() {
                comm_args.push(l_str[i].clone());
            }
            let child = Command::new(command)
                .stdout::<File>(file_out)
                .stderr::<File>(file_err)
                .args(comm_args)
                .spawn()?;
            childs.push(child);
        } else {
            let l_str = command.split(' ').map(|x| x.to_string()).collect::<Vec<_>>();
            let command = &l_str[0];
            let mut comm_args = Vec::new();
            for i in 1..l_str.len() {
                comm_args.push(l_str[i].clone());
            }
            let output = Command::new(command)
                .stdout::<File>(file_out)
                .stderr::<File>(file_err)
                .args(comm_args)
                .output()?;
            println!("output={:?}", output);
        }
        i_command += 1;
    }
    //
    // Running the commands iteratively
    //
    let mut var_results : Vec<ResultSingleRun> = Vec::new();
    for iter in 0..config.n_iter {
        let var_result = execute_and_estimate_runtime(iter, &config)?;
        var_results.push(var_result);
    }
    Ok(())
}
