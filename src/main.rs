mod challenges;
mod utils;

fn main() {
    let arg = std::env::args().nth(1).expect("No argument provided");

    match arg.as_str() {
        "password_hashing" => challenges::password_hashing::run(),
        "help_me_unpack" => challenges::help_me_unpack::run(),
        "backup_restore" => challenges::backup_restore::run(),
        "brute_force_zip" => challenges::brute_force_zip::run(),
        _ => panic!("Unknown challenge"),
    }
}
