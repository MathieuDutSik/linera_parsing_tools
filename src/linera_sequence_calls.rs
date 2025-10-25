extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;
mod common;
use common::{execute_command_general, kill_processes, read_config_file};
use serde::Deserialize;
use std::process::Child;

#[derive(Deserialize)]
struct Entry {
    nature: String,
    #[serde(default)]
    directory: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    environments: Vec<String>,
    #[serde(default)]
    kill_names: Vec<String>,
}

#[derive(Deserialize)]
struct Config(Vec<Entry>);


fn execute_command(i_command: usize, entry: &Entry, childs: &mut Vec<Child>) -> anyhow::Result<()> {
    let directory = &entry.directory;
    let command = &entry.command;
    let file_out_str = &entry.stdout;
    let file_err_str = &entry.stderr;
    let environments = &entry.environments;
    println!(
        "i_command={} directory={} command={} stdout={} stderr={}",
        i_command, directory, command, file_out_str, file_err_str,
    );
    //
    let directory: Option<String> = Some(directory.clone());
    execute_command_general(command,
                            directory,
                            file_out_str.to_string(),
                            file_err_str.to_string(),
                            &environments,
                            childs)?;
    Ok(())
}





fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().into_iter().collect::<Vec<_>>();
    let n_arg = arguments.len();
    if n_arg != 2 {
        println!("Program is used as");
        println!("linera_sequence_calls [FileI]");
        std::process::exit(1)
    }
    let file_input = &arguments[1];
    println!("file_input={}", file_input);
    let config = read_config_file::<Config>(file_input)?;
    let n_command = config.0.len();
    println!("n_command={}", n_command);
    let mut childs = Vec::new();
    for (i_command, entry) in config.0.into_iter().enumerate() {
        let nature = entry.nature.clone();
        println!("i_command={i_command} nature={nature}");

        match nature.trim() {
            "execute_command" => {
                execute_command(i_command, &entry, &mut childs)?;
            },
            "kill_processes" => {
                kill_processes(&entry.kill_names);
            },
            _ => {
                anyhow::bail!("No matching entry for nature={nature}");
            }
        }
    }

    Ok(())
}
