use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
enum Block {
    Data(Vec<Vec<(String, i32)>>),
    Nonce(i32),
}

fn has_leading_zeros(hash: &[u8], bits: usize) -> bool {
    let full_bytes = bits / 8;
    let remaining_bits = bits % 8;

    for i in 0..full_bytes {
        if hash[i] != 0 {
            return false;
        }
    }

    if remaining_bits > 0 {
        let mask = 0xFF << (8 - remaining_bits);
        if hash[full_bytes] & mask != 0 {
            return false;
        }
    }

    true
}

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("mini_miner");
    let problem = client.get_problem();
    let data = problem["block"]["data"].clone();
    let difficulty = problem["difficulty"].as_i64().unwrap() as usize;

    let mut solution = json!({
      "nonce": 0
    });

    for nonce in 0..1_000_000 {
        // use IndexMap to preserve order, as with json is not guaranteed
        let mut block = IndexMap::new();
        block.insert("data".to_string(), json!(data));
        block.insert("nonce".to_string(), json!(nonce));

        let full_dynamic_json: Value = Value::Object(block.clone().into_iter().collect());
        let serialized = serde_json::to_string(&full_dynamic_json).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();
        if has_leading_zeros(&hash, difficulty) {
            println!("Found nonce: {}", nonce);
            solution["nonce"] = json!(nonce);
            client.submit_solution(solution);
            break;
        }
    }
}
