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
        "basic_face_detection" => challenges::basic_face_detection::run(),
        "visual_basic_math" => challenges::visual_basic_math::run(),
        "collision_course" => challenges::collision_course::run(),
        "reading_qr" => challenges::reading_qr::run(),
        _ => panic!("Unknown challenge"),
    }
}
