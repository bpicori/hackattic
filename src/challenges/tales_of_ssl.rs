use base64::Engine;
use openssl::{
    asn1::Asn1Time,
    bn::BigNum,
    hash::MessageDigest,
    pkey::PKey,
    x509::{
        X509, X509NameBuilder,
        extension::{BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName},
    },
};
use serde_json::json;

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("tales_of_ssl");

    let problem = client.get_problem();
    let private_key = problem["private_key"].as_str().unwrap();
    // decode private key from base64
    let private_key: Vec<u8> = base64::engine::general_purpose::STANDARD
        .decode(private_key)
        .unwrap();

    let domain = problem["required_data"]["domain"].as_str().unwrap();
    let serial_number = problem["required_data"]["serial_number"].as_str().unwrap();
    let mut country = problem["required_data"]["country"].as_str().unwrap();

    let pkey = PKey::private_key_from_der(&private_key).unwrap();

    // Subject/issuer
    let mut issuer_name = X509NameBuilder::new().unwrap();
    println!("Country: {}", country);
    if country == "Tokelau Islands" {
        country = "Tokelau";
    }

    if country == "Sint Maarten" {
        country = "Saint Martin (French part)";
    }

    if country == "Cocos Island" {
        country = "Cocos (Keeling) Islands";
    }

    if country == "Keeling Islands" {
        country = "Cocos (Keeling) Islands";
    }

    let country = nationify::by_country_name(country).unwrap();
    issuer_name
        .append_entry_by_text("C", country.iso_code)
        .unwrap();
    issuer_name.append_entry_by_text("CN", domain).unwrap();
    let issuer_name = issuer_name.build();

    // build cert
    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&issuer_name).unwrap();
    builder.set_issuer_name(&issuer_name).unwrap();
    builder.set_pubkey(&pkey).unwrap();

    // set serial number
    let serial_number = BigNum::from_hex_str(serial_number.trim_start_matches("0x") as &str)
        .unwrap()
        .to_asn1_integer()
        .unwrap();
    builder.set_serial_number(&serial_number).unwrap();

    // set validity
    builder
        .set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::days_from_now(365).unwrap())
        .unwrap();

    // set extensions
    let basic_constraints = BasicConstraints::new().critical().build().unwrap();
    builder.append_extension(basic_constraints).unwrap();

    let key_usage = KeyUsage::new()
        .digital_signature()
        .key_encipherment()
        .build()
        .unwrap();
    builder.append_extension(key_usage).unwrap();

    let ext_key_usage = ExtendedKeyUsage::new()
        .server_auth()
        .client_auth()
        .build()
        .unwrap();
    builder.append_extension(ext_key_usage).unwrap();

    let subject_alt_name = SubjectAlternativeName::new()
        .dns(domain)
        .build(&builder.x509v3_context(None, None))
        .unwrap();
    builder.append_extension(subject_alt_name).unwrap();

    // sign it with the private key
    builder.sign(&pkey, MessageDigest::sha256()).unwrap();
    let cert: X509 = builder.build();

    // export to DER
    let cert_der = cert.to_der().unwrap();

    // encode to base64
    let cert_der = base64::engine::general_purpose::STANDARD.encode(cert_der);

    // submit solution
    let solution = json!({
        "certificate": cert_der
    });
    client.submit_solution(solution);
}
