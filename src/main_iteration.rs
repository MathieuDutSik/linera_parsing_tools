extern crate serde;
use serde::{Deserialize};
use std::fs::File;
use std::io::BufReader;
use std::process::{
    Command,
};


#[derive(Deserialize)]
struct Config {
    commands: Vec<String>,
    critical_command: String,
    target_variables: Vec<String>,
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
    for command in config.commands {
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
                .output()?;
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
    Ok(())
}
