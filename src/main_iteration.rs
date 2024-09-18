extern crate serde;
use serde::{Deserialize};
use std::fs::File;
use std::io::BufReader;



#[derive(Deserialize)]
struct Config {
    commands: Vec<String>,
    critical_command: String,
    target_variables: Vec<String>,
}

fn main() {
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
    let file = File::open(file_input).expect("Failed to open file");
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader).expect("Failed to parse JSON");
    println!("commands={:?}", config.commands);
    for command in commands {
        
        
        
    }
}
