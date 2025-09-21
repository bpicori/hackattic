use std::io::Read;

use base64::{Engine, engine::general_purpose};
use flate2::read::GzDecoder;
use regex::Regex;
use serde_json::json;

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("backup_restore");

    let problem = client.get_problem();
    let b64 = problem["dump"].as_str().unwrap();

    let buf = general_purpose::STANDARD
        .decode(b64)
        .expect("expect base64");

    let mut d = GzDecoder::new(&buf[..]);
    let mut s = String::new();
    d.read_to_string(&mut s).expect("Failed to decompress");

    let re = Regex::new(r"COPY .+;\n([\s\S]*)\\\.").unwrap();
    let extracted_text = re.captures(&s).unwrap().get(1).unwrap().as_str();

    let mut socials: Vec<String> = Vec::new();
    for line in extracted_text.lines() {
        let columns: Vec<&str> = line.split('\t').collect();

        let status = columns[columns.len() - 1];
        if status == "alive" {
            socials.push(columns[3].to_string());
        }
    }

    let solution = json!({
        "alive_ssns": socials
    });

    client.submit_solution(solution);
}
