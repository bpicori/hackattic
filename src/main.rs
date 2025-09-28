mod challenges;
mod utils;

fn main() {
    let arg = std::env::args().nth(1).expect("No argument provided");

    match arg.as_str() {
        "password_hashing" => challenges::password_hashing::run(),
        "help_me_unpack" => challenges::help_me_unpack::run(),
        "backup_restore" => challenges::backup_restore::run(),
        "brute_force_zip" => challenges::brute_force_zip::run(),
        "mini_miner" => challenges::mini_miner::run(),
        "tales_of_ssl" => challenges::tales_of_ssl::run(),
        "jotting_jwts" => challenges::jotting_jwts::run(),
        _ => panic!("Unknown challenge"),
    }
}
