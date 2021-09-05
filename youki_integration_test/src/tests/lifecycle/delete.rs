use super::get_result_from_output;
use crate::support::get_runtime_path;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::TestResult;

pub fn delete(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("delete")
        .arg(id)
        .spawn()
        .expect("failed to execute delete command")
        .wait_with_output();
    get_result_from_output(res)
}
