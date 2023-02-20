use crate::utils::get_state;
use anyhow::{anyhow, Result};
use std::path::Path;
use test_framework::TestResult;

pub fn state(project_path: &Path, id: &str) -> TestResult {
    match get_state(id, project_path) {
        Result::Ok((stdout, stderr)) => {
            if stderr.contains("Error") || stderr.contains("error") {
                TestResult::Failed(anyhow!("Error :\nstdout : {}\nstderr : {}", stdout, stderr))
            } else {
                // confirm that the status is stopped, as this is executed after the kill command
                if !(stdout.contains(&format!(r#""id": "{id}""#))
                    && stdout.contains(r#""status": "stopped""#))
                {
                    TestResult::Failed(anyhow!("Expected state stopped, got : {}", stdout))
                } else {
                    TestResult::Passed
                }
            }
        }
        Result::Err(e) => TestResult::Failed(e.context("failed to get container state")),
    }
}
