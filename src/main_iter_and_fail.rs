extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use serde::Deserialize;
use std::fs::File;
use std::process::Command;

use common::{read_config_file, make_file_available};

#[derive(Deserialize)]
struct Config {
    command: String,
    n_iter: usize,
    stop_at_one_failure: bool,
    stop_at_one_success: bool,
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
    let full_command = config.command;
    println!("full_command={:?}", full_command);
    let l_str = full_command
        .split(' ')
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    let command = &l_str[0];
    //
    // Now running
    //
    let n_iter = config.n_iter;
    let mut n_fail = 0;
    for iter in 0..n_iter {
        println!("iter={} / {} n_fail={}", iter, n_iter, n_fail);
        let file_out_str = format!("OUT_ITER_AND_FAIL_{}.out", iter);
        let file_err_str = format!("OUT_ITER_AND_FAIL_{}.err", iter);
        make_file_available(&file_out_str)?;
        make_file_available(&file_err_str)?;
        let file_out = File::create(file_out_str)?;
        let file_err = File::create(file_err_str)?;
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
        let code = output.status.code().unwrap();
        println!("code={:?}", code);
        if code > 0 {
            n_fail += 1;
            if config.stop_at_one_failure {
                println!("We reached one failure, end the computation");
                return Ok(());
            }
        } else {
            if config.stop_at_one_success {
                println!("We reached one success, end the computation");
                return Ok(());
            }
        }
    }
    println!("n_iter={} n_fail={}", n_iter, n_fail);
    println!("------ The runs have been done successfully -------");
    Ok(())
}
