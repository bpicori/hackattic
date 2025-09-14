use base64::Engine;
use hex;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use scrypt;
use sha2::{Digest, Sha256};

pub fn run() {
    let password = "rosebud7415";
    let salt_encoded = "UskMKp/7WvMEPokF4I8=";
    let rounds = 650_000;
    let log_n = 18;
    let r = 8;
    let p = 2;

    let salt_decoded = base64::engine::general_purpose::STANDARD
        .decode(salt_encoded)
        .unwrap();

    // SHA256
    let mut hasher = Sha256::new();
    hasher.update(password);
    let sha256_result = hasher.finalize();
    println!("SHA-256: {:x}", sha256_result);

    // --- HMAC-SHA256 ---
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(&salt_decoded).expect("HMAC can take key of any size");
    mac.update(password.as_bytes());
    let result = mac.finalize();
    let hmac_bytes = result.into_bytes();
    println!("HMAC-SHA256: {}", hex::encode(hmac_bytes));

    // PBKDF2-HMAC-SHA256
    let mut pbkdf2_result = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        &salt_decoded,
        rounds,
        &mut pbkdf2_result,
    );
    println!("PBKDF2-SHA256: {}", hex::encode(pbkdf2_result));

    // Scrypt
    let mut scrypt_result = [0u8; 32];
    let params = scrypt::Params::new(log_n, r, p, 32).expect("invalid params");
    scrypt::scrypt(
        password.as_bytes(),
        &salt_decoded,
        &params,
        &mut scrypt_result,
    )
    .expect("scrypt failed");
    println!("Scrypt: {}", hex::encode(scrypt_result));
}
