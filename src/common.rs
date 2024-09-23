extern crate chrono;
extern crate serde_json;
extern crate yaml_rust;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;

pub fn get_float(input: &str) -> f64 {
    let input = input.trim_matches(|c| c == '"').to_string();
    input.parse::<f64>().expect("A float")
}

pub struct ReadData {
    pub min_time: usize,
    pub entries: Vec<Vec<(usize, String)>>,
    pub le: Option<f64>,
}

fn get_time_string(ti: DateTime<Utc>) -> String {
    let year = ti.year();
    let month = ti.month();
    let day = ti.day();
    let hour = ti.hour();
    let minute = ti.minute();
    let second = ti.second();
    let year = format!("{}", year);
    let month = format!("{:02}", month);
    let day = format!("{:02}", day);
    let hour = format!("{:02}", hour);
    let minute = format!("{:02}", minute);
    let second = format!("{:02}", second);
    format!("{}-{}-{}T{}:{}:{}Z", year, month, day, hour, minute, second)
}

pub fn get_time_string_lower(time: DateTime<Utc>) -> String {
    let time = time - Duration::seconds(2);
    get_time_string(time)
}

pub fn get_time_string_upper(time: DateTime<Utc>) -> String {
    let time = time + Duration::seconds(2);
    get_time_string(time)
}

pub fn get_request_string(datetime: DateTime<Utc>) -> String {
    let year = datetime.year();
    let month = datetime.month();
    let day = datetime.day();
    let hour = datetime.hour();
    let min = datetime.minute();
    let sec = datetime.second();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

pub fn read_key(key: &str, l_job_name: &Vec<String>, start_time: &str, end_time: &str) -> ReadData {
    let mut min_time: usize = usize::MAX;
    let mut map_job_name = BTreeMap::<String, usize>::new();
    let n_job = l_job_name.len();
    for (idx, job_name) in l_job_name.iter().enumerate() {
        map_job_name.insert(job_name.clone(), idx);
    }
    let request = format!(
        "http://localhost:9090/api/v1/query_range?query={}&start={}&end={}&step=1s",
        key, start_time, end_time
    );
    println!("request={}", request);
    let output = Command::new("curl")
        .arg(request)
        .output()
        .expect("Failed to execute command");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let mut entries = Vec::new();
    for _ in 0..n_job {
        entries.push(Vec::new());
    }
    let le = None;
    for entry in v["data"]["result"].as_array().unwrap() {
        let job_name: String = entry["metric"]["job"].to_string();
        let job_name = job_name.trim_matches(|c| c == '"').to_string();
        let mut values = BTreeMap::new();
        for value in entry["values"].as_array().unwrap() {
            let unix_time: usize = value[0].as_u64().unwrap() as usize;
            let inst_value: String = value[1].to_string();
            if unix_time < min_time {
                min_time = unix_time;
            }
            values.insert(unix_time, inst_value);
        }
        let mut values_vect = Vec::new();
        for (k, v) in values {
            values_vect.push((k, v));
        }
        if let Some(pos) = map_job_name.get(&job_name) {
            entries[*pos] = values_vect;
        }
    }
    ReadData {
        min_time,
        entries,
        le,
    }
}

pub fn read_linera_keys() -> (Vec<String>, Vec<String>) {
    let request = "http://localhost:9090/api/v1/label/__name__/values".to_string();
    let output = Command::new("curl")
        .arg(request)
        .output()
        .expect("Failed to execute curl command for getting ListLabel");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let mut variables = Vec::new();
    let linera = "linera_";
    for entry in v["data"].as_array().unwrap() {
        let entry: String = entry.to_string();
        let entry = entry.trim_matches(|c| c == '"').to_string();
        if entry.starts_with(linera) {
            let entry = entry[linera.len()..].to_string();
            variables.push(entry);
        }
    }
    let mut l_keys_counter = Vec::new();
    let mut l_keys_hist = Vec::new();
    for var in variables {
        let test1 = var.ends_with("_sum");
        let test2 = var.ends_with("_bucket");
        let test3 = var.ends_with("_count");
        if !test1 && !test2 && !test3 {
            l_keys_counter.push(var.clone());
        }
        if test1 {
            let len = var.len();
            let var = var[..len - 4].to_string();
            l_keys_hist.push(var);
        }
    }
    (l_keys_counter, l_keys_hist)
}
