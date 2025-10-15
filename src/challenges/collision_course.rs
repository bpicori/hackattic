use base64::Engine;
use serde_json::json;

fn execute_fastcoll() -> std::process::Output {
    // Get current directory and user/group IDs
    let current_dir = std::env::current_dir().unwrap();
    let current_dir_str = current_dir.to_str().unwrap();

    let volume_mount = format!("{}:/work", current_dir_str);

    let mut binding = std::process::Command::new("docker");
    let command = binding
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(&volume_mount)
        .arg("-w")
        .arg("/work")
        .arg("brimstone/fastcoll")
        .arg("--prefixfile")
        .arg("./data/prefix.txt")
        .arg("-o")
        .arg("./data/file1.bin")
        .arg("./data/file2.bin");

    // print command
    println!("Executing command: {:?}", command);
    let output = command.output().unwrap();

    return output;
}

pub fn run() {
    let client = crate::utils::hackattic_client::HackatticClient::new("collision_course");

    let problem = client.get_problem();
    let prefix = problem["include"].as_str().unwrap();

    // save prefix to file
    std::fs::write("./data/prefix.txt", prefix).unwrap();

    let output = execute_fastcoll();
    if !output.status.success() {
        println!(
            "fastcoll failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!("fastcoll failed");
    }
    println!(
        "fastcoll output: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let file1 = std::fs::read("./data/file1.bin").unwrap();
    let file2 = std::fs::read("./data/file2.bin").unwrap();

    // encode to base64
    let file1 = base64::engine::general_purpose::STANDARD.encode(file1);
    let file2 = base64::engine::general_purpose::STANDARD.encode(file2);

    let solution = json!({
      "files": [file1, file2]
    });

    client.submit_solution(solution);
}
