use base64::{Engine, engine::general_purpose};

pub fn run() {
    let b64 = "gswHh8MpZ92NrQAANtKnQ2wmAdrxzX9AQH/N8doBJmw=";
    let buf = general_purpose::STANDARD.decode(b64).expect("Invalid");
    println!("Bytes: {:?}", buf);

    let mut offset = 0;
    let int_val = i32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
    offset += 4;

    let uint_val = u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
    offset += 4;

    let short_val = i16::from_le_bytes(buf[offset..offset + 2].try_into().unwrap());
    offset += 4;

    let float_val = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
    offset += 4;

    let double_val = f64::from_le_bytes(buf[offset..offset + 8].try_into().unwrap());
    offset += 8;

    let double_be_val = f64::from_be_bytes(buf[offset..offset + 8].try_into().unwrap());

    println!("i32: {}", int_val);
    println!("u32: {}", uint_val);
    println!("i16: {}", short_val);
    println!("f32: {}", float_val);
    println!("f64: {}", double_val);
    println!("f64 (big-endian): {}", double_be_val);
}
