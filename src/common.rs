#![allow(dead_code)]

extern crate chrono;
extern crate serde_json;
extern crate yaml_rust;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde_json::Value;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::process::Command;
use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::path::Path;

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

pub fn get_duration_as_string(duration: Duration) -> String {
    let num_sec = duration.num_seconds();
    let num_sec = if num_sec > 0 {
        num_sec
    } else {
        -num_sec
    };
    let n_day = num_sec / 86400;
    let n_hour = (num_sec % 86400) / 3600;
    let n_min = (num_sec % 3600) / 60;
    if n_day > 0 {
        return format!("n_day={} n_hour={} n_min={}", n_day, n_hour, n_min);
    }
    if n_hour > 0 {
        return format!("n_hour={} n_min={}", n_hour, n_min);
    }
    format!("n_min={}", n_min)
}


pub fn get_unit_of_key(key: &str) -> String {
    if key.ends_with("latency") {
        return " ms".to_string();
    }
    if key.ends_with("runtime") {
        return " ms".to_string();
    }
    "".to_string()
}

pub fn nice_float_str(value: f64) -> String {
    if value > 1 as f64 {
        return format!("{:.2}", value);
    }
    format!("{:.5}", value)
}

pub fn make_file_available(file_name: &str) -> anyhow::Result<()> {
    let mut iter = 0;
    loop {
        let first_free_file_attempt = if iter == 0 {
            file_name.to_string()
        } else {
            format!("{}_V{}", file_name, iter)
        };
        if !Path::new(&first_free_file_attempt).exists() {
            if file_name != &first_free_file_attempt {
                std::fs::rename(file_name, &first_free_file_attempt)?;
            }
            return Ok(());
        }
        iter += 1;
    }
}

pub fn read_config_file<Config: DeserializeOwned>(file_input: &str) -> anyhow::Result<Config> {
    let file = File::open(file_input)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader)?;
    Ok(config)
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
//    println!("request={}", request);
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

pub fn get_key_delta(data: &ReadData, i_job: usize) -> Option<f64> {
    let len = data.entries[i_job].len();
    if len > 0 {
        let val_upp = get_float(&data.entries[i_job][len - 1].1);
        let val_low = get_float(&data.entries[i_job][0].1);
        let delta = val_upp - val_low;
        Some(delta)
    } else {
        None
    }
}



pub fn read_distribution_key(key: &str, l_job_name: &Vec<String>, start_time: &str, end_time: &str) -> Vec<Vec<f64>> {
    let key_sum = format!("linera_{}_sum", key);
    let key_count = format!("linera_{}_count", key);
    let data_sum = read_key(&key_sum, l_job_name, start_time, end_time);
    let data_count = read_key(&key_count, l_job_name, start_time, end_time);
    let n_job = l_job_name.len();
    let mut results = Vec::new();
    for i in 0..n_job {
        let len_sum = data_sum.entries[i].len();
        let len_count = data_count.entries[i].len();
        if len_sum != len_count {
            println!("len_sum={} len_count={}", len_sum, len_count);
            panic!("Not our assumptions");
        }
        let mut vals = Vec::new();
        for idx in 1..len_sum {
            let value1 = &data_sum.entries[i][idx].1;
            let value1 = get_float(value1);
            let value0 = &data_sum.entries[i][idx - 1].1;
            let value0 = get_float(value0);
            let delta_sum = value1 - value0;
            let count1 = &data_count.entries[i][idx].1;
            let count0 = &data_count.entries[i][idx - 1].1;
            if count1 != count0 {
                let count1 = get_float(count1);
                let count0 = get_float(count0);
                let delta_count = count1 - count0;
                let val = delta_sum / delta_count;
                vals.push(val);
            }
        }
        results.push(vals);
    }
    results
}

pub fn read_lines_of_file(file_name: &String) -> Vec<String> {
    let file = File::open(file_name).expect("A file");
    let reader = BufReader::new(file);
    //
    let mut lines = Vec::new();
    for pre_line in reader.lines() {
        let line = pre_line.expect("line");
        lines.push(line);
    }
    lines
}

pub fn create_single_line(lines: Vec<String>) -> String {
    let mut single_line = String::new();
    for i in 0..lines.len() {
        if i>0 {
            single_line += " ";
        }
        single_line += &lines[i];
    }
    single_line
}

pub fn get_benchmark_average_metric_mus(single_line: &str, target: &str) -> Option<f64> {
    let target_ext = format!("{} ", target);
//    println!("single_line={}", single_line);
//    println!("target_ext=\"{}\"", target_ext);
    let l_str_a = single_line.split(&target_ext).map(|x| x.to_string()).collect::<Vec<_>>();
    if l_str_a.len() < 2 {
        return None;
    }
    let sec_str_a = &l_str_a[1];
    let sep_str_micros = "µs";
    let sep_str_millis = "ms";
    let sec_str_b = sec_str_a.replace("[", " ");
    let sec_str_c = sec_str_b.replace("]", " ");
    let mut metrics_mus = Vec::new();
    let l_str_b = sec_str_c.split(" ").map(|x| x.to_string()).collect::<Vec<_>>();
    for i in 0..l_str_b.len() {
        if l_str_b[i] == sep_str_micros {
            let metric_mus : f64 = l_str_b[i - 1].parse().unwrap();
            metrics_mus.push(metric_mus);
        }
        if l_str_b[i] == sep_str_millis {
            let metric_millis : f64 = l_str_b[i - 1].parse().unwrap();
            let metric_mus = metric_millis * 1000.0;
            metrics_mus.push(metric_mus);
        }
    }
    if metrics_mus.len() < 3 {
        panic!("We should have at least 3 entries");
    }
    Some(metrics_mus[1])
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

