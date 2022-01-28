use super::get_result_from_output;
use crate::utils::{kill_container};
use std::path::Path;
use test_framework::TestResult;

pub fn kill(project_path: &Path, id: &str) -> TestResult {
    let res = kill_container(id, project_path)
        .expect("failed to execute kill command")
        .wait_with_output();
    get_result_from_output(res)
}
