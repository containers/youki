use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[test]
fn main() {
    let current_dir_path_result = env::current_dir();
    let current_dir_path = match current_dir_path_result {
        Ok(path_buf) => path_buf,
        Err(_) => panic!("directory is not found"),
    };
    let youki_path = current_dir_path.join(PathBuf::from("youki"));
    let status = Command::new(youki_path)
        .stdout(Stdio::null())
        .arg("-h")
        .status()
        .expect("failed to execute process");
    assert!(status.success());
}
