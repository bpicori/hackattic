use crossbeam_channel::{Receiver, Sender, unbounded};
use std::fs;
use std::thread;

pub fn run() {
    let file = fs::read("data/package_1.zip").unwrap();
    let is_zip = crate::utils::zip::check_if_zip(&file);
    if !is_zip {
        panic!("The file provided is not a zip file");
    }

    let charset: Vec<char> = ('a'..='z').chain('0'..='9').collect();

    let (tx_main, rx_main): (Sender<String>, Receiver<String>) = unbounded();
    let files = crate::utils::zip::extract_all_files(&file);
    let (_, secret_content, crc32) = files
        .iter()
        .find(|(filename, _, _)| filename == "secret.txt")
        .unwrap()
        .clone();
    let charset_clone = charset.clone();

    // Spawn a producer thread
    thread::spawn(move || {
        println!("Password generator thread started.");
        for length in 4..=6 {
            println!("Generating passwords of length {}", length);
            let mut indices = vec![0; length];

            loop {
                let password: String = indices.iter().map(|&i| charset_clone[i]).collect();
                // Send password to main thread
                tx_main.send(password.clone()).unwrap();

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


    // Spawn 8 worker threads
    for i in 0..num_workers {
        // Clone the receiver for each worker
        let rx_worker = rx_main.clone();
        let content = secret_content.clone();
        let handle = thread::spawn(move || {
            println!("Worker {} started.", i);
            // The loop will automatically break when the sender is dropped and the channel is empty.
            while let Ok(password) = rx_worker.recv() {
                if crate::utils::zip::verify_zip_crypto_password(&content, &password, crc32) {
                    println!("Found password: {}", password);
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

    println!("All threads have finished.");
}
