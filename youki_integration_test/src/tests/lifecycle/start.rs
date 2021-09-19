use super::get_result_from_output;
use crate::utils::get_runtime_path;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::TestResult;

pub fn start(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("runtime"))
        .arg("start")
        .arg(id)
        .spawn()
        .expect("failed to execute start command")
        .wait_with_output();
    get_result_from_output(res)
}
