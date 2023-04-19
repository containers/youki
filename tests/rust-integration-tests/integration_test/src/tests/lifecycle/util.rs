use std::{io, process};
use test_framework::{testable::TestError, TestResult};

pub fn get_result_from_output(res: io::Result<process::Output>) -> TestResult<()> {
    match res {
        Ok(output) => {
            let stderr = String::from_utf8(output.stderr).unwrap();
            if stderr.contains("Error") || stderr.contains("error") {
                let stdout = String::from_utf8(output.stdout).unwrap();
                Err(TestError::Failed(anyhow::anyhow!(
                    "Error :\nstdout : {}\nstderr : {}",
                    stdout,
                    stderr
                )))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(TestError::Failed(anyhow::Error::new(e))),
    }
}
