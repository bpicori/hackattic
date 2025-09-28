use std::sync::{Arc, Mutex};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use warp::{Filter, reply::json};

#[derive(Serialize, Deserialize)]
struct Response {
    solution: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    append: Option<String>,
    nbf: Option<i64>,
}

const URL: &str = "https://c8a9248290ec.ngrok-free.app";

async fn get_problem() -> String {
    let client = crate::utils::hackattic_client::HackatticClient::new("jotting_jwts");
    let problem = client.get_problem_async().await;
    let jwt_secret = problem["jwt_secret"].as_str().unwrap().to_string();
    return jwt_secret;
}

async fn start_challenge() {
    let client = crate::utils::hackattic_client::HackatticClient::new("jotting_jwts");
    client
        .submit_solution_async(json!({
          "app_url": URL
        }))
        .await;
}

#[tokio::main]
pub async fn run() {
    let solution = Arc::new(Mutex::new(String::new()));

    // get problem
    let jwt_secret = get_problem().await;
    println!("JWT Secret: {}", jwt_secret);

    // Define the hello world route
    let route = warp::post()
        .and(warp::path::end())
        .and(warp::body::bytes())
        .map(move |body: warp::hyper::body::Bytes| {
            let solution = Arc::clone(&solution);
            let jwt_secret = jwt_secret.clone();

            let token = String::from_utf8(body.to_vec()).unwrap();

            let mut validation = Validation::new(Algorithm::HS256);
            validation.required_spec_claims = Default::default();

            let token = decode::<Claims>(
                &token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &validation,
            );

            if token.is_err() {
                println!("Invalid token: {:?}", token);
                return json(&Response {
                    solution: "Invalid Token".to_string(),
                });
            }

            let token = token.unwrap();

            // check nbf
            if let Some(nbf) = token.claims.nbf {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;

                if nbf > now {
                    println!("Token not yet valid");
                    return json(&Response {
                        solution: "Token not yet valid".to_string(),
                    });
                }
            }

            println!("Appending to solution: {:?}", token.claims.append);
            if token.claims.append.is_none() {
                let solution = solution.lock().unwrap();
                println!("RETURNING SOLUTION: {}", solution);
                return json(&Response {
                    solution: solution.clone(),
                });
            }

            let mut solution = solution.lock().unwrap();
            if let Some(ref append_str) = token.claims.append {
                *solution += append_str;
            }

            let response = Response {
                solution: solution.clone(),
            };

            json(&response)
        });

    println!("Starting server on http://127.0.0.1:3030");

    // sleep for 1 seconds
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // start challenge
    tokio::spawn(async move {
        start_challenge().await;
    });

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;
}
