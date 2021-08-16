use super::get_result_from_output;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use test_framework::TestResult;

pub fn delete(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(project_path.join(PathBuf::from("youki")))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-r")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("delete")
        .arg(id)
        .spawn()
        .expect("failed to execute delete command")
        .wait_with_output();
    get_result_from_output(res)
}
