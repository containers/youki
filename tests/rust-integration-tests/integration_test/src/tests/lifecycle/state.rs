use crate::utils::get_state;
use anyhow::anyhow;
use std::path::Path;
use test_framework::{testable::TestError, TestResult};

pub fn state(project_path: &Path, id: &str) -> TestResult<()> {
    match get_state(id, project_path) {
        Ok((stdout, stderr)) => {
            if stderr.contains("Error") || stderr.contains("error") {
                return Err(TestError::Failed(anyhow!(
                    "Error :\nstdout : {}\nstderr : {}",
                    stdout,
                    stderr
                )));
            }

            // confirm that the status is stopped, as this is executed after the kill command
            if !(stdout.contains(&format!(r#""id": "{id}""#))
                && stdout.contains(r#""status": "stopped""#))
            {
                Err(TestError::Failed(anyhow!(
                    "Expected state stopped, got : {}",
                    stdout
                )))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(TestError::Failed(
            e.context("failed to get container state"),
        )),
    }
}
