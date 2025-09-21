use crossbeam_channel::{Receiver, Sender, unbounded};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

pub fn run() {
    let file = fs::read("data/package_1.zip").unwrap();
    let is_zip = crate::utils::zip::check_if_zip(&file);
    if !is_zip {
        panic!("The file provided is not a zip file");
    }

    let charset: Vec<char> = ('a'..='z').chain('0'..='9').collect();

    // Counter for tracking password attempts (when actually tried, not generated)
    let password_counter = Arc::new(AtomicU64::new(0));
    let password_found = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    let (tx_main, rx_main): (Sender<String>, Receiver<String>) = unbounded();
    let files = crate::utils::zip::extract_all_files(&file);
    let (_, secret_content, crc32) = files
        .iter()
        .find(|(filename, _, _)| filename == "secret.txt")
        .unwrap()
        .clone();
    let charset_clone = charset.clone();

    // Spawn logging thread
    let counter_clone = Arc::clone(&password_counter);
    let found_flag_logger = Arc::clone(&password_found);
    let start_time_clone = start_time;
    thread::spawn(move || {
        let log_interval_secs = 2; // Change this to adjust logging frequency
        let mut last_count = 0u64;
        let mut last_time = start_time_clone;

        loop {
            thread::sleep(Duration::from_secs(log_interval_secs));

            // Check if password was found
            if found_flag_logger.load(Ordering::Relaxed) {
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

    // Spawn a producer thread
    let found_flag_producer = Arc::clone(&password_found);
    thread::spawn(move || {
        println!("Password generator thread started.");
        for length in 4..=6 {
            println!("Generating passwords of length {}", length);
            let mut indices = vec![0; length];

            loop {
                // Check if password was found
                if found_flag_producer.load(Ordering::Relaxed) {
                    println!("Password found, stopping generator.");
                    break;
                }

                let password: String = indices.iter().map(|&i| charset_clone[i]).collect();
                // Send password to main thread
                if tx_main.send(password.clone()).is_err() {
                    // Channel closed, workers are done
                    break;
                }

                // Increment indices (like base-36 counter)
                let mut pos = length as isize - 1;
                while pos >= 0 {
                    indices[pos as usize] += 1;
                    if indices[pos as usize] < charset_clone.len() {
                        break;
                    }
                    indices[pos as usize] = 0;
                    pos -= 1;
                }
                if pos < 0 {
                    break; // finished all passwords of this length
                }
            }
        }
        // Dropping the sender signals that no more messages will be sent.
        drop(tx_main);
    });

    let mut handles = vec![];
    let num_workers = num_cpus::get() - 1;

    // Spawn worker threads
    for i in 0..num_workers {
        // Clone the receiver for each worker
        let rx_worker = rx_main.clone();
        let content = secret_content.clone();
        let counter_worker = Arc::clone(&password_counter);
        let found_flag_worker = Arc::clone(&password_found);
        let handle = thread::spawn(move || {
            println!("Worker {} started.", i);
            // The loop will automatically break when the sender is dropped and the channel is empty.
            while let Ok(password) = rx_worker.recv() {
                // Increment counter when we actually TRY the password
                counter_worker.fetch_add(1, Ordering::Relaxed);

                if crate::utils::zip::verify_zip_crypto_password(&content, &password, crc32) {
                    println!("Found password: {}", password);
                    found_flag_worker.store(true, Ordering::Relaxed);
                    break;
                }
            }
            println!("Worker {} finished.", i);
        });
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

    println!("All threads have finished.");
    println!("Final statistics:");
    println!("  Total passwords tried: {}", format_number(final_count));
    println!("  Total time: {:.2} seconds", total_elapsed);
    println!("  Average rate: {}/sec", format_rate(final_rate));
}
