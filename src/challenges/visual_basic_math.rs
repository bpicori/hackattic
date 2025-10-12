use leptess::{LepTess, Variable};
use opencv::{
    core::{AlgorithmHint, BORDER_DEFAULT, Mat, Size},
    imgcodecs, imgproc,
};

pub fn run() {
    // Load the original image
    let img = imgcodecs::imread("data/math_1.jpeg", imgcodecs::IMREAD_COLOR).unwrap();

    // Before grayscale conversion
    let mut upscaled = Mat::default();
    imgproc::resize(
        &img,
        &mut upscaled,
        Size::new(1, 1),
        2.0, // Scale factor
        2.0,
        imgproc::INTER_CUBIC,
    )
    .unwrap();

    // Convert to grayscale
    let mut gray = Mat::default();
    imgproc::cvt_color(
        &img,
        &mut gray,
        imgproc::COLOR_BGR2GRAY,
        0,
        AlgorithmHint::ALGO_HINT_ACCURATE,
    )
    .unwrap();

    // Apply Gaussian blur to smooth edges
    let mut blur = Mat::default();
    imgproc::gaussian_blur(
        &gray,
        &mut blur,
        Size::new(1, 1),
        0.0,
        0.0,
        BORDER_DEFAULT,
        AlgorithmHint::ALGO_HINT_ACCURATE,
    )
    .unwrap();

    // Apply adaptive threshold (handles colors)
    let mut thresh = Mat::default();
    imgproc::adaptive_threshold(
        &blur,
        &mut thresh,
        255.0,
        imgproc::ADAPTIVE_THRESH_GAUSSIAN_C,
        imgproc::THRESH_BINARY_INV,
        11,
        2.0,
    )
    .unwrap();

    // Save the preprocessed image for Tesseract
    imgcodecs::imwrite(
        "data/math_1.jpeg",
        &thresh,
        &opencv::core::Vector::new(),
    )
    .unwrap();

    // Now OCR
    let mut lt = LepTess::new(None, "eng+equ").unwrap();

    lt.set_variable(Variable::TesseditCharWhitelist, "0123456789+-×÷")
        .unwrap();

    lt.set_image("data/math_1_clean.png").unwrap();
    let text = lt.get_utf8_text().unwrap();

    println!("Detected text:\n{}", text);
}
