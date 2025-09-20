use std::{fs, io::Cursor};

use zip::ZipArchive;

const ZIP_FILE_SIGNATURE: &[u8; 4] = b"PK\x03\x04";
const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const ZIP_CRYPTO_HEADER_SIZE: usize = 12;

// ZIP Layout
// [Local File Header 1][File Data 1][Data Descriptor?]
// [Local File Header 2][File Data 2][Data Descriptor?]
// ...
// [Central Directory]
// [End of Central Directory Record]

/// Represents the End of Central Directory (EOCD) record in a ZIP file.
/// EOCD is the metadata about the archive
///
/// Layout in bytes (all little-endian):
///
/// | Offset | Size | Field                  |
/// |--------|------|------------------------|
/// | 0      | 4    | Signature (0x06054b50) |
/// | 4      | 2    | Disk number            |
/// | 6      | 2    | Disk where CD starts   |
/// | 8      | 2    | Entries on this disk   |
/// | 10     | 2    | Total entries          |
/// | 12     | 4    | Size of CD (bytes)     |
/// | 16     | 4    | Offset of CD           |
/// | 20     | 2    | Comment length (n)     |
/// | 22     | n    | Comment                |
/// |--------|------|------------------------|
///
#[derive(Debug)]
struct EndOfCentralDirectory {
    /// 2 bytes @ offset 4
    disk_number: u16,
    /// 2 bytes @ offset 6
    start_disk: u16,
    /// 2 bytes @ offset 8
    entries_on_disk: u16,
    /// 2 bytes @ offset 10
    total_entries: u16,
    /// 4 bytes @ offset 12
    central_directory_size: u32,
    /// 4 bytes @ offset 16
    central_directory_offset: u32,
    /// 2 bytes @ offset 20
    comment_length: u16,
    /// n bytes @ offset 22
    comment: String,
}
fn read_eocd(bytes: &Vec<u8>) -> Option<EndOfCentralDirectory> {
    let mut pos = None;
    let mut i = bytes.len().saturating_sub(4);

    while i > 0 {
        if &bytes[i..(i + 4)] == EOCD_SIGNATURE {
            pos = Some(i);
            break;
        }
        i -= 1;
    }

    let pos = pos?;
    let disk_number = u16::from_le_bytes(bytes[pos + 4..pos + 6].try_into().unwrap());
    let start_disk = u16::from_le_bytes(bytes[pos + 6..pos + 8].try_into().unwrap());
    let entries_on_disk = u16::from_le_bytes(bytes[pos + 8..pos + 10].try_into().unwrap());
    let total_entries = u16::from_le_bytes(bytes[pos + 10..pos + 12].try_into().unwrap());
    let central_directory_size = u32::from_le_bytes(bytes[pos + 12..pos + 16].try_into().unwrap());
    let central_directory_offset =
        u32::from_le_bytes(bytes[pos + 16..pos + 20].try_into().unwrap());
    let comment_length = u16::from_le_bytes(bytes[pos + 20..pos + 22].try_into().unwrap());

    let comment_bytes = &bytes[pos + 22..pos + 22 + comment_length as usize];
    let comment = String::from_utf8_lossy(comment_bytes).into_owned();

    Some(EndOfCentralDirectory {
        disk_number,
        start_disk,
        entries_on_disk,
        total_entries,
        central_directory_size,
        central_directory_offset,
        comment_length,
        comment,
    })
}

/// Represents a single file entry in the Central Directory
///
/// Layout in bytes (all little-endian):
///
/// | Offset | Size | Field                   | Notes                            
/// |--------|------|-------------------------| ---------------------------------
/// | 0      | 4    | Signature (0x02014b50)  |
/// | 4      | 2    | Version made by         |
/// | 6      | 2    | Version needed to extract |
/// | 8      | 2    | General purpose flag     | check if encrypted or not
/// | 10     | 2    | Compression method      | 0 -> no_compression, 8 -> deflate
/// | 12     | 2    | Last mod file time       |
/// | 14     | 2    | Last mod file date       |
/// | 16     | 4    | CRC-32                  
/// | 20     | 4    | Compressed size         |
/// | 24     | 4    | Uncompressed size       |
/// | 28     | 2    | Filename length (n)     |
/// | 30     | 2    | Extra field length (m)   |
/// | 32     | 2    | File comment length (k) |
/// | 42     | 4    | Local header offset     |
/// | 46     | n    | Filename                |
/// | 46+n   | m    | Extra field              |
/// | 46+n+m | k    | File comment            |
/// |--------|------|-------------------------|
///
#[derive(Debug)]
struct CentralDirectoryEntry {
    /// File name
    filename: String,
    /// 2 bytes @ offset 8
    general_purpose_flag: u16,
    /// 2 bytes @ offset 10
    compression_method: u16,
    /// 2 bytes @ offset 10
    last_mod_time: u16,
    /// 2 bytes @ offset 16
    crc32: u32,
    /// 4 bytes @ offset 20
    compressed_size: u32,
    /// 4 bytes @ offset 24
    uncompressed_size: u32,
    /// 4 bytes @ offset 42
    local_header_offset: u32,
}

fn read_central_directory_entry(bytes: &[u8], offset: usize) -> (CentralDirectoryEntry, usize) {
    // signature
    let sig = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!(sig, 0x02014b50, "Invalid CD entry signature");

    let general_purpose_flag =
        u16::from_le_bytes(bytes[offset + 8..offset + 10].try_into().unwrap());

    let compression_method =
        u16::from_le_bytes(bytes[offset + 10..offset + 12].try_into().unwrap());

    let last_mod_time = u16::from_le_bytes(bytes[offset + 12..offset + 14].try_into().unwrap());

    let crc32 = u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap());

    let compressed_size = u32::from_le_bytes(bytes[offset + 20..offset + 24].try_into().unwrap());
    let uncompressed_size = u32::from_le_bytes(bytes[offset + 24..offset + 28].try_into().unwrap());

    let filename_len =
        u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;
    let extra_len =
        u16::from_le_bytes(bytes[offset + 30..offset + 32].try_into().unwrap()) as usize;
    let comment_len =
        u16::from_le_bytes(bytes[offset + 32..offset + 34].try_into().unwrap()) as usize;

    let filename_start = offset + 46;
    let filename_end = filename_start + filename_len;
    let filename = String::from_utf8_lossy(&bytes[filename_start..filename_end]).into_owned();

    let local_header_offset =
        u32::from_le_bytes(bytes[offset + 42..offset + 46].try_into().unwrap());

    let next_offset = filename_end + extra_len + comment_len;

    (
        CentralDirectoryEntry {
            filename,
            general_purpose_flag,
            last_mod_time,
            crc32,
            compression_method,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        },
        next_offset,
    )
}

fn read_file_data<'a>(bytes: &'a [u8], cde: &'a CentralDirectoryEntry) -> &'a [u8] {
    let offset = cde.local_header_offset as usize;

    let filename_len =
        u16::from_le_bytes(bytes[offset + 26..offset + 28].try_into().unwrap()) as usize;
    let extra_len =
        u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;

    let data_start = offset + 30 + filename_len + extra_len;
    let data_end = data_start + cde.compressed_size as usize;

    return &bytes[data_start..data_end];
}

fn extract_all_files(bytes: &[u8], eocd: &EndOfCentralDirectory) {
    let mut offset = eocd.central_directory_offset as usize;

    for _ in 0..eocd.total_entries {
        let (entry, next_offset) = read_central_directory_entry(&bytes, offset);
        println!("{}", entry.filename);
        if entry.filename == "secret.txt" {
            let file_data = read_file_data(bytes, &entry);

            println!(
                "is encrypted: {:?}",
                is_encrypted(entry.general_purpose_flag)
            );
            println!("compression method {:?}", entry.compression_method);
            println!("compressed: {:?}", entry.compressed_size);
            println!("uncompressed: {:?}", entry.uncompressed_size);
            println!("crc32: {:?}", entry.crc32);

            let enc_header = &file_data[..12];
            let password = "a";
            let valid = verify_zip_crypto_password(enc_header, password, entry.last_mod_time);

            println!("is_valid: {:?}", valid);
            println!(
                "File: {:?}, Content: {:?}",
                entry.filename,
                String::from_utf8_lossy(file_data),
            );
        }

        offset = next_offset
    }
}

fn check_if_zip(bytes: &Vec<u8>) -> bool {
    return &bytes[0..4] == ZIP_FILE_SIGNATURE;
}

fn is_encrypted(general_purpose_flag: u16) -> bool {
    return (general_purpose_flag & 0x0001) != 0;
}

fn verify_zip_crypto_password(enc_header: &[u8], password: &str, mod_time: u16) -> bool {
    if enc_header.len() < ZIP_CRYPTO_HEADER_SIZE {
        return false;
    }

    // Initialize ZipCrypto keys
    let mut keys = (0x12345678, 0x23456789, 0x34567890);

    // PART OF ZIPCRYPTO ALGO
    //This function updates a CRC32 checksum for a single byte of data.
    // It performs bitwise operations on the current CRC value and the new byte
    fn crc32_update(mut crc: u32, byte: u8) -> u32 {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
        crc
    }
    // PART OF ZIPCRYPTO ALGO
    fn update_keys(keys: &mut (u32, u32, u32), byte: u8) {
        keys.0 = crc32_update(keys.0, byte);
        keys.1 = keys.1.wrapping_add(keys.0 & 0xff);
        keys.1 = keys.1.wrapping_mul(134775813).wrapping_add(1);
        keys.2 = crc32_update(keys.2, (keys.1 >> 24) as u8);
    }

    // PART OF ZIPCRYPTO ALGO
    fn decrypt_byte(keys: &(u32, u32, u32)) -> u8 {
        let temp = keys.2 | 2;
        (((temp.wrapping_mul(temp ^ 1)) >> 8) & 0xff) as u8
    }

    // Initialize keys with password
    for byte in password.bytes() {
        update_keys(&mut keys, byte);
    }

    let mut decrypted = [0u8; ZIP_CRYPTO_HEADER_SIZE];
    for i in 0..ZIP_CRYPTO_HEADER_SIZE {
        let k = decrypt_byte(&keys);
        decrypted[i] = enc_header[i] ^ k;
        update_keys(&mut keys, decrypted[i]);
    }

    // Check byte: last byte of decrypted header should equal high byte of modification time
    let check_byte = decrypted[11];
    let expected_check = (mod_time >> 8) as u8;

    return check_byte == expected_check;
}

pub fn run() {
    let file = fs::read("data/encrypted.zip").unwrap();
    let is_zip = check_if_zip(&file);
    if !is_zip {
        panic!("The file provided is not a zip file");
    }

    let eocd = read_eocd(&file).unwrap();
    println!("{:?}", eocd);

    extract_all_files(&file, &eocd);
}
