use crate::support::get_runtime_path;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::TestResult;

// There are still some issues here
// in case we put stdout and stderr as piped
// the youki process created halts indefinitely
// which is why we pass null, and use wait instead of wait_with_output
pub fn create(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("-r")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("create")
        .arg(id)
        .arg("--bundle")
        .arg(project_path.join("integration-workspace").join("bundle"))
        .spawn()
        .expect("Cannot execute create command")
        .wait();
    match res {
        io::Result::Ok(status) => {
            if status.success() {
                TestResult::Ok
            } else {
                TestResult::Err(anyhow::anyhow!(
                    "Error : create exited with nonzero status : {}",
                    status
                ))
            }
        }
        io::Result::Err(e) => TestResult::Err(anyhow::Error::new(e)),
    }
}
