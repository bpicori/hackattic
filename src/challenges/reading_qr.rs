use image;
use rqrr;

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("reading_qr");
    let problem = client.get_problem();
    let image_url = problem["image_url"].as_str().unwrap();
    let image_bytes = client.download_file(image_url);
    std::fs::write("./data/qr_code.png", image_bytes).unwrap();

    let img = image::open("./data/qr_code.png").unwrap().to_luma8();
    let mut img = rqrr::PreparedImage::prepare(img);
    let grids = img.detect_grids();

    let (_meta, content) = grids[0].decode().unwrap();

    let solution = serde_json::json!({
        "code": content
    });

    let client = crate::utils::hackattic_client::HackatticClient::new("reading_qr");
    client.submit_solution(solution);
}
