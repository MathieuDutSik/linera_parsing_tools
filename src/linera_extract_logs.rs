extern crate chrono;
extern crate serde_json;
extern crate yaml_rust;
mod common;
use serde::Deserialize;

use common::{read_config_file, read_lines_of_file};

#[derive(Deserialize)]
struct Config {
    log_file: String,
    entries: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args = std::env::args();
    let mut arguments = Vec::new();
    for argument in args {
        println!("argument={argument}");
        arguments.push(argument);
    }
    let n_arg = arguments.len();
    if n_arg != 2{
        println!("Program is used as");
        println!("linera_extract_logs [FileI]");
        std::process::exit(1)
    }
    let file_input = &arguments[1];
    let config = read_config_file::<Config>(file_input)?;

    let lines = read_lines_of_file(&config.log_file);
    for entry in config.entries {
        let mut n_ms = 0 as f64;
        let mut count = 0;
        for line in &lines {
            if line.ends_with("ms") {
                let l_str = line.split(&entry).collect::<Vec<_>>();
                if l_str.len() == 2 {
                    let sec_ent = l_str[1];
                    let sec_sel = sec_ent
                        .chars()
                        .filter(|c| c.is_numeric())
                        .collect::<String>();
                    let value = sec_sel.parse::<u64>().expect("a numerical value");
                    n_ms += value as f64;
                    count += 1;
                }
            }
        }
        if count > 0 {
            let avg = n_ms / (count as f64);
            println!("The entry <<{}>> has an average of {} ms", entry, avg);
        } else {
            println!("The entry <<{}>> did not match anything in the log", entry);
        }
    }
    Ok(())
}
