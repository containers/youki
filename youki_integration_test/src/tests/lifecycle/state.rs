use crate::support::get_runtime_path;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::TestResult;

pub fn state(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-r")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("state")
        .arg(id)
        .spawn()
        .expect("failed to execute state command")
        .wait_with_output();
    match res {
        io::Result::Ok(output) => {
            let stderr = String::from_utf8(output.stderr).unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            if stderr.contains("Error") || stderr.contains("error") {
                TestResult::Err(anyhow::anyhow!(
                    "Error :\nstdout : {}\nstderr : {}",
                    stdout,
                    stderr
                ))
            } else {
                // confirm that the status is stopped, as this is executed after the kill command
                if !(stdout.contains(&format!(r#""id": "{}""#, id))
                    && stdout.contains(r#""status": "stopped""#))
                {
                    TestResult::Err(anyhow::anyhow!("Expected state stopped, got : {}", stdout))
                } else {
                    TestResult::Ok
                }
            }
        }
        io::Result::Err(e) => TestResult::Err(anyhow::Error::new(e)),
    }
}
