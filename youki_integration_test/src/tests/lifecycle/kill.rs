use super::get_result_from_output;
use crate::support::get_runtime_path;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use test_framework::TestResult;

// By experimenting, somewhere around 50 is enough for youki process
// to get the kill signal and shut down
// here we add a little buffer time as well
const SLEEP_TIME: u64 = 75;

pub fn kill(project_path: &Path, id: &str) -> TestResult {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("kill")
        .arg(id)
        .arg("9")
        .spawn()
        .expect("failed to execute kill command")
        .wait_with_output();
    // sleep a little, so the youki process actually gets the signal and shuts down
    // otherwise, the tester moves on to next tests before the youki has gotten signal, and delete test can fail
    sleep(Duration::from_millis(SLEEP_TIME));
    get_result_from_output(res)
}
