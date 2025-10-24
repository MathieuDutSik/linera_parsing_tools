extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use common::read_config_file;
use serde::Deserialize;
use std::fs::File;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct Config {
    directories: Vec<String>,
    commands: Vec<String>,
    stdouts: Vec<String>,
    stderrs: Vec<String>,
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
    println!("file_input={}", file_input);
    let config = read_config_file::<Config>(file_input)?;
    let n_command = config.directories.len();
    println!("n_command={}", n_command);
    for i_command in 0..n_command {
        let directory = config.directories[i_command].clone();
        let command = config.commands[i_command].clone();
        let stdout = config.stdouts[i_command].clone();
        let stderr = config.stderrs[i_command].clone();
        println!(
            "i_command={} directory={} command={} stdout={} stderr={}",
            i_command, directory, command, stdout, stderr
        );
        //
        let file_out = File::create(stdout)?;
        let file_err = File::create(stderr)?;
        let l_str = command
            .split(' ')
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        let the_command = &l_str[0];
        let mut comm_args = Vec::new();
        for i in 1..l_str.len() {
            comm_args.push(l_str[i].clone());
        }
        println!("the_command={} comm_args={:?}", the_command, comm_args);
        let path = Path::new(&directory);
        let output = Command::new(the_command)
            .current_dir(path)
            .stdout::<File>(file_out)
            .stderr::<File>(file_err)
            .args(comm_args)
            .output()?;
        println!("i_command={} output={:?}", i_command, output);
    }

    Ok(())
}
