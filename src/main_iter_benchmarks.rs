extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::process::Command;
use sysinfo::{ProcessExt, System, SystemExt};
use common::{read_lines_of_file, create_single_line, get_benchmark_average_metric_mus};

#[derive(Deserialize)]
struct Config {
    commands: Vec<String>,
    n_iter: usize,
    n_skip: usize,
    targets: Vec<String>,
}



fn get_metrics_mus(config: &Config, iter: usize) -> Vec<f64> {
    let n_target = config.targets.len();
    let mut vec = vec![None; n_target];
    let mut i_command = 0;
    for command in &config.commands {
        println!("i_command={} iter={}", i_command, iter);
        let file_out_str = format!("OUT_ITER_BENCHMARK_{}_{}.out", iter, i_command);
        let file_out = File::create(file_out_str.clone()).expect("A file to have been created");
        let file_err_str = format!("OUT_ITER_BENCHMARK_{}_{}.out", iter, i_command);
        let file_err = File::create(file_err_str).expect("A file to have been created");
        let l_str = command
            .split(' ')
	    .map(|x| x.to_string())
            .collect::<Vec<_>>();
        let raw_command = &l_str[0];
        let mut comm_args = Vec::new();
        for i in 1..l_str.len() {
            comm_args.push(l_str[i].clone());
        }
        let output = Command::new(raw_command)
            .stdout::<File>(file_out)
            .stderr::<File>(file_err)
            .args(comm_args)
            .output()
            .expect("Output to have been created");
        println!("output={:?}", output);
        let lines = read_lines_of_file(&file_out_str);
        let single_line = create_single_line(lines);
        for i_target in 0..n_target {
            let result = get_benchmark_average_metric_mus(&single_line, &config.targets[i_target]);
            if let Some(metric_mus) = result {
                vec[i_target] = Some(metric_mus);
            }
        }
        i_command += 1;
    }
    let mut vec_ret = Vec::new();
    for entry in vec {
        if let Some(metric_mus) = entry {
            vec_ret.push(metric_mus);
        } else {
            panic!("The metric should have been obtained");
        }
    }
    vec_ret
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

    let mut results = Vec::new();
    for iter in 0..config.n_iter {
        let result = get_metrics_mus(&config, iter);
        results.push(result);
    }
    let n_target = config.targets.len();
    let n_samp = config.n_iter - config.n_skip;
    for i_target in 0..n_target {
        let mut sum_val = 0 as f64;
        for iter in config.n_skip..config.n_iter {
            let val = results[iter][i_target];
            sum_val += val;
        }
        let avg = sum_val / (n_samp as f64);
        println!("target={} avg={}", config.targets[i_target], avg);
    }
    println!("------ The runs have been done successfully -------");
    Ok(())
}
