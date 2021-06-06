use std::env;
use std::process::{Command, Stdio};

#[test]
fn main() {
    let path_result = env::current_dir();
    let path = match path_result {
        Ok(path) => path.display().to_string(),
        Err(_) => panic!("Path is not found"),
    };
    let status = Command::new(path + "/target/x86_64-unknown-linux-gnu/debug/youki")
        .stdout(Stdio::null())
        .arg("-h")
        .status()
        .expect("failed to execute process");
    assert!(status.success());
}
