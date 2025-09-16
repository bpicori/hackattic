use std::io::Read;

use base64::{Engine, engine::general_purpose};
use flate2::read::GzDecoder;
use regex::Regex;
use serde_json::json;

const GET_URL: &str =
    "https://hackattic.com/challenges/backup_restore/problem?access_token=a6af29a286fe2625";
const POST_URL: &str =
    "https://hackattic.com/challenges/backup_restore/solve?access_token=a6af29a286fe2625";

pub fn run() {
    let b64 = get_dump();
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

    post_response(socials);
}

fn get_dump() -> String {
    let resp = reqwest::blocking::get(GET_URL)
        .expect("failed to fetch")
        .json::<serde_json::Value>()
        .expect("failed to parse json");

    resp["dump"].as_str().unwrap().to_string()
}

fn post_response(socials: Vec<String>) {
    let body = json!({
      "alive_ssns": socials
    });

    let resp = reqwest::blocking::Client::new()
        .post(POST_URL)
        .json(&body)
        .send()
        .expect("Failed to send POST");

    let status = resp.status();
    let text = resp.text().expect("Failed to read response body");
    println!("Status: {}", status);
    println!("Body: {}", text);
}
