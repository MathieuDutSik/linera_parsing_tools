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
    name: String,
    #[serde(default = "Entry::default_nature")]
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

impl Entry {
    fn default_nature() -> String {
        "execute_command".to_string()
    }
}


#[derive(Deserialize)]
struct Config(Vec<Entry>);


fn execute_command(i_command: usize, entry: &Entry, childs: &mut Vec<Child>) -> anyhow::Result<()> {
    let name = &entry.name;
    println!("i_command={i_command} (execute_command) name={name}");
    let directory = &entry.directory;
    let command = &entry.command;
    let mut file_out_str = entry.stdout.to_string();
    let mut file_err_str = entry.stderr.to_string();
    if file_out_str.is_empty() {
        file_out_str = format!("COMM_DEFAULT_{i_command}_out");
    }
    if file_err_str.is_empty() {
        file_err_str = format!("COMM_DEFAULT_{i_command}_err");
    }
    let environments = &entry.environments;
    //
    let directory = if !directory.is_empty() {
        Some(directory.clone())
    } else {
        None
    };
    execute_command_general(command,
                            directory,
                            file_out_str,
                            file_err_str,
                            environments,
                            childs)?;
    Ok(())
}

fn do_kill_processes(i_command: usize, entry: &Entry) -> anyhow::Result<()> {
    let name = &entry.name;
    println!("i_command={i_command} (kill_processes) name={name}");
    kill_processes(&entry.kill_names);
    Ok(())
}




fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
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
        println!("---------------------------- {i_command} / {n_command} ----------------------------");

        match nature.trim() {
            "execute_command" => {
                execute_command(i_command, &entry, &mut childs)?;
            },
            "kill_processes" => {
                do_kill_processes(i_command, &entry)?;
            },
            _ => {
                anyhow::bail!("No matching entry for nature={nature}");
            }
        }
    }

    Ok(())
}
