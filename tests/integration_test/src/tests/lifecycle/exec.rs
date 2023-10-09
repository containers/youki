use super::get_result_from_output;
use crate::utils::get_runtime_path;
use anyhow::Result;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::assert_result_eq;

pub fn exec(
    project_path: &Path,
    id: &str,
    exec_cmd: Vec<&str>,
    expected_output: Option<&str>,
) -> Result<()> {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("runtime"))
        .arg("exec")
        .arg(id)
        .args(exec_cmd)
        .spawn()
        .expect("failed to execute exec command")
        .wait_with_output();
    if let Some(expect) = expected_output {
        let act = String::from_utf8(res.as_ref().unwrap().stdout.clone()).unwrap();
        assert_result_eq!(expect, act.as_str(), "unexpected stdout.").unwrap();
    }
    get_result_from_output(res)
}
