use std::env;

const BASE_URL: &str = "https://hackattic.com/challenges";

pub struct HackatticClient {
    challenge_name: String,
    access_token: String,
}

impl HackatticClient {
    pub fn new(challenge_name: &str) -> Self {
        // Load environment variables from .env file
        dotenv::dotenv().ok();

        let access_token =
            env::var("ACCESS_TOKEN").expect("ACCESS_TOKEN must be set in environment or .env file");

        Self {
            challenge_name: challenge_name.to_string(),
            access_token,
        }
    }

    /// Get the problem data from the Hackattic API
    pub fn get_problem(&self) -> serde_json::Value {
        let url = format!(
            "{}/{}/problem?access_token={}",
            BASE_URL, self.challenge_name, self.access_token
        );

        reqwest::blocking::get(&url)
            .expect("Failed to fetch problem")
            .json::<serde_json::Value>()
            .expect("Failed to parse JSON")
    }

    /// Submit a solution to the Hackattic API
    pub fn submit_solution(&self, solution: serde_json::Value) {
        let url = format!(
            "{}/{}/solve?access_token={}",
            BASE_URL, self.challenge_name, self.access_token
        );

        let resp = reqwest::blocking::Client::new()
            .post(&url)
            .json(&solution)
            .send()
            .expect("Failed to send POST");

        let status = resp.status();
        let text = resp.text().expect("Failed to read response body");
        println!("Status: {}", status);
        println!("Response: {}", text);
    }

    /// Download a file from a URL
    pub fn download_file(&self, url: &str) -> Vec<u8> {
        reqwest::blocking::get(url)
            .expect("Failed to download file")
            .bytes()
            .expect("Failed to read file bytes")
            .to_vec()
    }
}
