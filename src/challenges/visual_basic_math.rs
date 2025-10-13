use serde_json::json;

const IMAGE_PATH: &str = "./data/math.jpeg";


fn sanitize_and_parse(s: &str) -> (Option<char>, Option<f64>) {
    let operator = s.chars().next();
    let mut clean_string = s.chars().skip(1).collect::<String>();

    clean_string = clean_string
        .chars()
        .map(|c| match c {
            '０' => '0',
            '１' => '1',
            '２' => '2',
            '３' => '3',
            '４' => '4',
            '５' => '5',
            '６' => '6',
            '７' => '7',
            '８' => '8',
            '９' => '9',
            _ => c,
        })
        .collect();

    return (operator, clean_string.parse::<f64>().ok());
}

fn calculate(lines: Vec<String>) -> i64 {
    let first_line = lines[0].clone();
    let (first_line_operator, first_line_number) = sanitize_and_parse(&first_line);
    let first_line_number = first_line_number.unwrap();
    let first_line_operator = first_line_operator.unwrap();
    let mut result = if first_line_operator == '-' {
        -first_line_number
    } else {
        first_line_number
    };

    for line in lines.iter().skip(1) {
        let (operator, number) = sanitize_and_parse(line);
        let number = number.unwrap();
        let operator = operator.unwrap();

        let old_result = result;
        if operator == '+' {
            result += number;
            println!("{} + {} = {}", old_result, number, result);
        } else if operator == '-' {
            result -= number;
            println!("{} - {} = {}", old_result, number, result);
        } else if operator == '×' {
            result *= number;
            println!("{} × {} = {}", old_result, number, result);
        } else if operator == '÷' {
            // Float division, then floor (round down)
            result = (result / number).floor();
            println!("{} ÷ {} = {}", old_result, number, result);
        } else {
            println!(
                "Unknown operator: '{}' (char code: {})",
                operator, operator as u32
            );
        }
    }

    // Convert final result to i64, flooring to ensure rounding down
    return result.floor() as i64;
}

fn call_ocr_model() -> String {
    println!("Calling OCR model...");

    let mut paddle_ocr_command = std::process::Command::new("paddleocr");
    paddle_ocr_command.arg("ocr");
    paddle_ocr_command.arg("-i");
    paddle_ocr_command.arg(IMAGE_PATH);
    paddle_ocr_command.arg("--use_doc_orientation_classify");
    paddle_ocr_command.arg("False");
    paddle_ocr_command.arg("--use_doc_unwarping");
    paddle_ocr_command.arg("False");
    paddle_ocr_command.arg("--use_textline_orientation");
    paddle_ocr_command.arg("False");
    paddle_ocr_command.arg("--save_path");
    paddle_ocr_command.arg("./data/output");

    paddle_ocr_command.output().unwrap();
    println!("OCR model called successfully");

    let json = std::fs::read_to_string("./data/output/math_res.json").unwrap();

    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rec_texts = json["rec_texts"].as_array().unwrap();
    let rec_texts = rec_texts
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect::<Vec<&str>>();
    let rec_texts = rec_texts.join("\n");
    return rec_texts;
}

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("visual_basic_math");
    let problem = client.get_problem();
    let image_url = problem["image_url"].as_str().unwrap();
    let image_bytes = client.download_file(image_url);
    std::fs::write(IMAGE_PATH, image_bytes).unwrap();

    let response = call_ocr_model();
    let lines: Vec<String> = response.lines().map(|s| s.to_string()).collect();

    println!("Lines:");
    for line in lines.iter() {
        println!("{}", line);
    }

    println!("------------------");
    let result = calculate(lines);
    println!("------------------");
    println!("Result: {}", result);

    let solution = json!({
        "result": result
    });

    client.submit_solution(solution);
}
