use std::fs;

fn generate_passwords(
    charset: &[char],
    length: usize,
    prefix: &mut String,
    enc_data: &[u8],
    expected_crc32: u32,
) {
    if prefix.len() == length {
        println!("Checking password: {}", prefix);
        if crate::utils::zip::verify_zip_crypto_password(enc_data, prefix, expected_crc32) {
            panic!("Found password: {}", prefix);
        }
        return;
    }

    for &c in charset {
        prefix.push(c);
        generate_passwords(charset, length, prefix, enc_data, expected_crc32);
        prefix.pop(); // Backtrack
    }
}

pub fn run() {
    let file = fs::read("data/package.zip").unwrap();
    let is_zip = crate::utils::zip::check_if_zip(&file);
    if !is_zip {
        panic!("The file provided is not a zip file");
    }

    let charset: Vec<char> = ('a'..='z').chain('0'..='9').collect();
    println!("charset: {:?}", charset);

    let files = crate::utils::zip::extract_all_files(&file);
    for (filename, content, crc32, ) in files {
        if filename == "secret.txt" {
            let password = "vnxgz";
            let valid_password =
                crate::utils::zip::verify_zip_crypto_password(&content, &password, crc32);
            println!("{}: {}", filename, valid_password);
        }
    }
}
