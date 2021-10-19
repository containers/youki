use std::{io, process};
use test_framework::TestResult;

pub fn get_result_from_output(res: io::Result<process::Output>) -> TestResult {
    match res {
        io::Result::Ok(output) => {
            let stderr = String::from_utf8(output.stderr).unwrap();
            if stderr.contains("Error") || stderr.contains("error") {
                let stdout = String::from_utf8(output.stdout).unwrap();
                TestResult::Failed(anyhow::anyhow!(
                    "Error :\nstdout : {}\nstderr : {}",
                    stdout,
                    stderr
                ))
            } else {
                TestResult::Passed
            }
        }
        io::Result::Err(e) => TestResult::Failed(anyhow::Error::new(e)),
    }
}
