use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Helper functions for human-readable formatting
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_rate(rate: f64) -> String {
    if rate >= 1_000_000.0 {
        format!("{:.1}M", rate / 1_000_000.0)
    } else if rate >= 1_000.0 {
        format!("{:.1}K", rate / 1_000.0)
    } else {
        format!("{:.0}", rate)
    }
}

fn spawn_password_generator(
    charset: Vec<char>,
    tx_main: Sender<String>,
    password_found: Arc<AtomicBool>,
    shutdown_signal: Arc<AtomicBool>,
) {
    let found_flag_producer = Arc::clone(&password_found);
    let shutdown_signal_producer = Arc::clone(&shutdown_signal);
    thread::spawn(move || {
        println!("Password generator thread started.");
        for length in 4..=6 {
            println!("Generating passwords of length {}", length);
            let mut indices = vec![0; length];

            loop {
                // Check if password was found or shutdown signal received
                if found_flag_producer.load(Ordering::Relaxed)
                    || shutdown_signal_producer.load(Ordering::Relaxed)
                {
                    println!("Stopping generator (password found or shutdown signal received).");
                    break;
                }

                let password: String = indices.iter().map(|&i| charset[i]).collect();
                // Send password to main thread
                if tx_main.send(password.clone()).is_err() {
                    // Channel closed, workers are done
                    break;
                }

                // Increment indices (like base-36 counter)
                let mut pos = length as isize - 1;
                while pos >= 0 {
                    indices[pos as usize] += 1;
                    if indices[pos as usize] < charset.len() {
                        break;
                    }
                    indices[pos as usize] = 0;
                    pos -= 1;
                }
                if pos < 0 {
                    break; // finished all passwords of this length
                }
            }
            println!("Finished generating passwords of length {}", length);
        }
        // Dropping the sender signals that no more messages will be sent.
        drop(tx_main);
    });
}

fn create_worker_handle(
    worker_id: usize,
    rx_worker: Receiver<String>,
    secret_content: Vec<u8>,
    crc32: u32,
    password_counter: Arc<AtomicU64>,
    password_found: Arc<AtomicBool>,
    shutdown_signal: Arc<AtomicBool>,
    found_password: Arc<Mutex<String>>,
    decrypted_content: Arc<Mutex<Vec<u8>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        println!("Worker {} started.", worker_id);
        // The loop will automatically break when the sender is dropped and the channel is empty.
        while let Ok(password) = rx_worker.recv() {
            // Check for shutdown signal before processing
            if shutdown_signal.load(Ordering::Relaxed) {
                println!("Worker {} received shutdown signal.", worker_id);
                break;
            }

            if password_found.load(Ordering::Relaxed) {
                println!("Worker {} received found signal.", worker_id);
                break;
            }

            // Increment counter when we actually TRY the password
            password_counter.fetch_add(1, Ordering::Relaxed);

            if crate::utils::zip::verify_zip_crypto_password(&secret_content, &password, crc32) {
                println!("Found password: {}", password);

                // Decrypt the file content
                let decrypted =
                    crate::utils::zip::decrypt_zip_crypto_content(&secret_content, &password);

                // Store the password and decrypted content
                if let Ok(mut pwd) = found_password.lock() {
                    *pwd = password.clone();
                }
                if let Ok(mut content_guard) = decrypted_content.lock() {
                    *content_guard = decrypted;
                }

                password_found.store(true, Ordering::Relaxed);
                break;
            }
        }
        println!("Worker {} finished.", worker_id);
    })
}

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("brute_force_zip");

    println!("Getting ZIP file URL from Hackattic API...");
    let problem = client.get_problem();
    let zip_url = problem["zip_url"].as_str().unwrap();
    println!("ZIP URL: {}", zip_url);

    println!("Downloading ZIP file...");
    let file = client.download_file(zip_url);
    let is_zip = crate::utils::zip::check_if_zip(&file);
    if !is_zip {
        panic!("The downloaded file is not a ZIP file");
    }
    println!("ZIP file downloaded successfully ({} bytes)", file.len());

    let charset: Vec<char> = ('a'..='z').chain('0'..='9').collect();

    let password_counter = Arc::new(AtomicU64::new(0));
    let password_found = Arc::new(AtomicBool::new(false));
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_signal_clone = Arc::clone(&shutdown_signal);
    let start_time = Instant::now();

    // Shared state for storing the found password and decrypted content
    let found_password = Arc::new(Mutex::new(String::new()));
    let decrypted_content = Arc::new(Mutex::new(Vec::<u8>::new()));

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C, shutting down gracefully...");
        shutdown_signal_clone.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl+C handler");

    let (tx_main, rx_main): (Sender<String>, Receiver<String>) = unbounded();
    let files = crate::utils::zip::extract_all_files(&file);
    let (_, secret_content, crc32) = files
        .iter()
        .find(|(filename, _, _)| filename == "secret.txt")
        .unwrap()
        .clone();

    // Spawn logging thread
    let counter_clone = Arc::clone(&password_counter);
    let found_flag_logger = Arc::clone(&password_found);
    let shutdown_signal_logger = Arc::clone(&shutdown_signal);
    let start_time_clone = start_time;
    thread::spawn(move || {
        let log_interval_secs = 2; // Change this to adjust logging frequency
        let mut last_count = 0u64;
        let mut last_time = start_time_clone;

        loop {
            thread::sleep(Duration::from_secs(log_interval_secs));

            // Check if password was found or shutdown signal received
            if found_flag_logger.load(Ordering::Relaxed)
                || shutdown_signal_logger.load(Ordering::Relaxed)
            {
                break;
            }

            let current_count = counter_clone.load(Ordering::Relaxed);
            let current_time = Instant::now();

            // Calculate rates
            let total_elapsed = start_time_clone.elapsed().as_secs_f64();
            let interval_elapsed = current_time.duration_since(last_time).as_secs_f64();

            let avg_rate = if total_elapsed > 0.0 {
                current_count as f64 / total_elapsed
            } else {
                0.0
            };

            let interval_rate = if interval_elapsed > 0.0 {
                (current_count - last_count) as f64 / interval_elapsed
            } else {
                0.0
            };

            println!(
                "Passwords tried: {} | Avg rate: {}/sec | Current rate: {}/sec",
                format_number(current_count),
                format_rate(avg_rate),
                format_rate(interval_rate)
            );

            // Update for next iteration
            last_count = current_count;
            last_time = current_time;
        }
    });

    // Spawn password generator thread
    spawn_password_generator(
        charset.clone(),
        tx_main,
        Arc::clone(&password_found),
        Arc::clone(&shutdown_signal),
    );

    let mut handles = vec![];
    let num_workers = num_cpus::get() - 1;

    // Spawn worker threads
    for i in 0..num_workers {
        // Clone the receiver for each worker
        let rx_worker = rx_main.clone();
        let handle = create_worker_handle(
            i,
            rx_worker,
            secret_content.clone(),
            crc32,
            Arc::clone(&password_counter),
            Arc::clone(&password_found),
            Arc::clone(&shutdown_signal),
            Arc::clone(&found_password),
            Arc::clone(&decrypted_content),
        );
        handles.push(handle);
    }

    // Wait for all worker threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    // Final statistics
    let final_count = password_counter.load(Ordering::Relaxed);
    let total_elapsed = start_time.elapsed().as_secs_f64();
    let final_rate = if total_elapsed > 0.0 {
        final_count as f64 / total_elapsed
    } else {
        0.0
    };

    let was_shutdown = shutdown_signal.load(Ordering::Relaxed);
    let was_found = password_found.load(Ordering::Relaxed);

    println!("All threads have finished.");
    if was_shutdown {
        println!("Program was interrupted by user (Ctrl+C).");
    } else if was_found {
        println!("Password was found successfully!");

        // Print the found password and decrypted content
        if let Ok(pwd) = found_password.lock() {
            if !pwd.is_empty() {
                println!("Password: {}", pwd);
            }
        }

        if let Ok(content) = decrypted_content.lock() {
            if !content.is_empty() {
                println!("Decrypted content:");
                match String::from_utf8(content.clone()) {
                    Ok(text) => {
                        println!("{}", text);
                        println!("Submitting solution to Hackattic API...");
                        let solution = json!({
                            "secret": text.trim()
                        });
                        client.submit_solution(solution);
                    }
                    Err(_) => {
                        panic!("Failed to decode decrypted content as UTF-8");
                    }
                }
            }
        }
    } else {
        println!("Search completed without finding password.");
    }

    println!("Final statistics:");
    println!("  Total passwords tried: {}", format_number(final_count));
    println!("  Total time: {:.2} seconds", total_elapsed);
    println!("  Average rate: {}/sec", format_rate(final_rate));
}
