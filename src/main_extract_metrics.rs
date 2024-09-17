extern crate yaml_rust;
extern crate serde_json;
use std::time::Duration;
use std::process::Command;
use std::io::Read;
use yaml_rust::YamlLoader;
use serde_json::Value;
use std::collections::BTreeMap;

extern crate chrono;
use chrono::Utc;
//use chrono::Date;
use chrono::DateTime;
//use chrono::TimeZone;
use chrono::Datelike;
use chrono::Timelike;

fn get_request_string(datetime: DateTime<Utc>) -> String {
    let year = datetime.year();
    let month = datetime.month();
    let day = datetime.day();
    let hour = datetime.hour();
    let min = datetime.minute();
    let sec = datetime.second();
    format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hour, min, sec)
}

fn get_float(input: &str) -> f64 {
    let input = input.trim_matches(|c| c == '"').to_string();
    input.parse::<f64>().expect("A float")
}


struct ReadData {
    min_time: usize,
    entries: Vec<Vec<(usize,String)>>,
    le: Option<f64>,
}

fn read_key(key: &str, l_job_name: &Vec<String>, start_time: &str, end_time: &str) -> ReadData {
    let mut min_time : usize = usize::MAX;
    let mut map_job_name = BTreeMap::<String,usize>::new();
    let mut idx = 0;
    let n_job = l_job_name.len();
    for job_name in l_job_name {
        map_job_name.insert(job_name.clone(), idx);
        idx += 1;
    }
    let request = format!("http://localhost:9090/api/v1/query_range?query={}&start={}&end={}&step=1s", key, start_time, end_time);
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
        let job_name : String = entry["metric"]["job"].to_string();
        let job_name = job_name.trim_matches(|c| c == '"').to_string();
        let mut values = BTreeMap::new();
        for value in entry["values"].as_array().unwrap() {
            let unix_time : usize = value[0].as_u64().unwrap() as usize;
            let inst_value : String = value[1].to_string();
            if unix_time < min_time {
                min_time = unix_time;
            }
            values.insert(unix_time, inst_value);
        }
        let mut values_vect = Vec::new();
        for (k, v) in values {
            values_vect.push((k, v));
        }
        let pos : usize = *map_job_name.get(&job_name).unwrap();
        entries[pos] = values_vect;
    }
    ReadData { min_time, entries, le }
}


fn read_linera_keys() -> (Vec<String>, Vec<String>) {
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
        let entry : String = entry.to_string();
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
            let var = var[..len-4].to_string();
            l_keys_hist.push(var);
        }
    }
//    println!("l_keys_counter={:?}", l_keys_counter);
//    println!("");
//    println!("l_keys_hist={:?}", l_keys_hist);
    (l_keys_counter, l_keys_hist)
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
    let (l_keys_counter, l_keys_hist) = read_linera_keys();
    if n_arg == 1 {
        println!("Program is used as");
        println!("parsing_prometheus_run [FileI] [interval]");
        println!("or");
        println!("parsing_prometheus_run [FileI] [start] [end]");
        println!("");
        println!("");
        println!("   FileI: The input file to Prometheus, e.g. prometheus.yml");
        println!("interval: The time scale in second to search from");
        std::process::exit(1)
    }
    let program_name = arguments[0].clone();
    let prometheus_input = arguments[1].clone();
    println!("program_name={:?}", program_name);
    println!("prometheus_input={:?}", prometheus_input);
    let (start_time, end_time) = if arguments.len() == 3 {
        let interval = arguments[2].clone();
        println!("interval={:?}", interval);
        let interval = interval.parse::<u64>().expect("A u64 integer");
        //
        let end_time: DateTime<Utc> = Utc::now();
        let duration_to_subtract = Duration::from_secs(interval);
        let start_time = end_time - duration_to_subtract;
        (start_time, end_time)
    } else {
        let start_time = arguments[2].clone();
        let end_time = arguments[3].clone();
        println!("INPUT: start_time={:?}", start_time);
        println!("INPUT:   end_time={:?}", end_time);
        let start_time: DateTime<Utc> = start_time.parse::<DateTime<Utc>>().expect("A UTC time (start)");
        let end_time: DateTime<Utc> = end_time.parse::<DateTime<Utc>>().expect("A UTC time (start)");
        (start_time, end_time)
    };
    let end_time = get_request_string(end_time);
    let start_time = get_request_string(start_time);
    println!("start_time={:?}", start_time);
    println!("  end_time={:?}", end_time);
    //
    // Reading the Prometheus input files and reading 
    //
    let mut f = std::fs::File::open(prometheus_input).expect("a filestream f");
    let mut contents = String::new();

    f.read_to_string(&mut contents)
        .expect("Unable to read file");

    let docs = YamlLoader::load_from_str(&contents).unwrap();
    let mut l_job_name = Vec::<String>::new();
    for entry in docs[0]["scrape_configs"].clone().into_iter() {
        let job_name = entry["job_name"].as_str().unwrap().to_string();
        println!("job_name={:?}", job_name);
        l_job_name.push(job_name.clone());
    }
    let n_job = l_job_name.len();
    println!("l_job_name={:?}", l_job_name);
    //
    println!("---------------- keys_counter -----------------");
    let mut n_counter_key_eff = 0;
    for key in l_keys_counter.clone() {
        let key = format!("linera_{}", key);
        let data = read_key(&key, &l_job_name, &start_time, &end_time);
        let mut n_write = 0;
        for i in 0..n_job {
            let len = data.entries[i].len();
            if len > 1 {
                n_write += 1;
                println!("key:{} job_name:{}", key, l_job_name[i]);
                for idx in 1..len {
                    let value1 = &data.entries[i][idx].1;
                    let value0 = &data.entries[i][idx-1].1;
                    if value1 != value0 {
                        let value1 = get_float(value1);
                        let value0 = get_float(value0);
                        let delta_val = value1 - value0;
                        let unix_time = data.entries[i][idx].0;
                        let delta_time = unix_time - data.min_time;
                        println!("delta_time={} delta_val={}", delta_time, delta_val);
                    }
                }
                let value_tot = &data.entries[i][len-1].1;
                let value_tot = get_float(value_tot);
                println!("    total_value={}", value_tot);
            }
        }
        if n_write > 0 {
            println!("");
            n_counter_key_eff += 1;
        }
    }
    println!("---------------- keys_histogram -----------------");
    let mut n_hist_key_eff = 0;
    for key in l_keys_hist.clone() {
        let key_count = format!("linera_{}_count", key);
        let data_count = read_key(&key_count, &l_job_name, &start_time, &end_time);
        let key_sum = format!("linera_{}_sum", key);
        let data_sum = read_key(&key_sum, &l_job_name, &start_time, &end_time);
        let mut n_write = 0;
        for i in 0..n_job {
            let len_sum = data_sum.entries[i].len();
            let len_count = data_count.entries[i].len();
            if len_sum != len_count {
                println!("len_sum={} len_count={}", len_sum, len_count);
                panic!("Not our assumptions");
            }
            if len_sum > 1 {
                n_write += 1;
                println!("key:{} job_name:{}", key, l_job_name[i]);
                for idx in 1..len_sum {
                    let value1 = &data_sum.entries[i][idx].1;
                    let value1 = get_float(value1);
                    let value0 = &data_sum.entries[i][idx-1].1;
                    let value0 = get_float(value0);
                    let delta_sum = value1 - value0;
                    let count1 = &data_count.entries[i][idx].1;
                    let count0 = &data_count.entries[i][idx-1].1;
                    if count1 != count0 {
                        let count1 = get_float(count1);
                        let count0 = get_float(count0);
                        let delta_count = count1 - count0;
                        let unix_time = data_sum.entries[i][idx-1].0;
                        let delta_time = unix_time - data_sum.min_time;
                        let avg = delta_sum / delta_count;
                        println!("delta_time={} avg={}     count={}", delta_time, avg, delta_count);
                    }
                }
                let count_tot = get_float(&data_count.entries[i][len_sum - 1].1);
                let value_tot = get_float(&data_sum.entries[i][len_sum - 1].1);
                let avg = value_tot / count_tot;
                println!("len_sum={} avg={}     count={} total={}", len_sum, avg, count_tot, value_tot);
            }
        }
        if n_write > 0 {
            println!("");
            n_hist_key_eff += 1;
        }
    }
    println!("n_counter_key_eff={} n_hist_key_eff={}", n_counter_key_eff, n_hist_key_eff);
}
