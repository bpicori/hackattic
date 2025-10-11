use std::fs;

use opencv::core::{MatTraitConst, Rect};
use opencv::{
    core::{Mat, Scalar, Size, Vector},
    imgcodecs, imgproc,
    objdetect::CascadeClassifier,
    prelude::CascadeClassifierTrait,
};
use serde_json::json;

const CASCADE_PATH: &str = "data/haarcascade_frontalface_alt2.xml";
const IMAGE_PATH: &str = "data/image.jpeg";
const OUTPUT_IMAGE_PATH: &str = "data/output.jpg";

pub fn run() {
    // --- 1. Download Image and Save ---
    let client = crate::utils::hackattic_client::HackatticClient::new("basic_face_detection");
    let problem = client.get_problem();
    let image_url = problem["image_url"].as_str().unwrap();
    let image_bytes = client.download_file(image_url);
    fs::write(IMAGE_PATH, image_bytes).unwrap();

    // --- 2. Load Again and Pre-process Image ---
    println!("Loading image from: {}", IMAGE_PATH);
    let original_img = match imgcodecs::imread(IMAGE_PATH, imgcodecs::IMREAD_COLOR) {
        Ok(m) => m,
        Err(_) => {
            eprintln!("Error: Could not read image at path: {}", IMAGE_PATH);
            return;
        }
    };

    let mut gray_img = Mat::default();
    // Convert to grayscale for the cascade classifier, apparently the model is trained on grayscale images
    imgproc::cvt_color(
        &original_img,
        &mut gray_img,
        imgproc::COLOR_BGR2GRAY,
        0,
        opencv::core::AlgorithmHint::ALGO_HINT_ACCURATE,
    )
    .unwrap();

    // // --- 3. Load the Cascade Classifier ---
    println!("Loading cascade classifier from: {}", CASCADE_PATH);
    let mut face_cascade = match CascadeClassifier::new(&CASCADE_PATH) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "Error: Could not load the cascade classifier from path: {}",
                CASCADE_PATH
            );
            eprintln!(
                "Make sure 'haarcascade_frontalface_default.xml' is in the correct location."
            );
            return;
        }
    };

    // --- 4. Detect Faces ---
    let mut faces = Vector::<Rect>::new();
    face_cascade
        .detect_multi_scale(
            &gray_img,
            &mut faces,
            1.1,
            5,
            0,
            Size::new(30, 30),
            Size::default(),
        )
        .unwrap();

    // --- 5. Calculate Face Tiles ---
    let mut face_tiles = Vec::new();
    let image_width = original_img.size().unwrap().width;
    let image_height = original_img.size().unwrap().height;
    for face in faces.iter() {
        let x = face.x;
        let y = face.y;

        let row = y / (image_height / 8);
        let col = x / (image_width / 8);
        face_tiles.push([row, col]);
    }

    // --- 6. Draw Rectangles for debugging ---
    let mut detected_faces_img = original_img.clone();
    let green = Scalar::new(0.0, 255.0, 0.0, 0.0);
    for face in faces.iter() {
        imgproc::rectangle(&mut detected_faces_img, face, green, 2, imgproc::LINE_8, 0).unwrap()
    }

    println!(
        "Saving image with highlighted faces to: {}",
        OUTPUT_IMAGE_PATH
    );
    imgcodecs::imwrite(OUTPUT_IMAGE_PATH, &detected_faces_img, &Vector::new()).unwrap();

    // --- 7. Submit Solution ---
    let solution = json!({
        "face_tiles": face_tiles
    });

    client.submit_solution(solution);
}
