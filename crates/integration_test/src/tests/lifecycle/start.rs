use super::get_result_from_output;
use std::path::Path;
use test_framework::TestResult;
use crate::utils::test_utils::start_container;

pub fn start(project_path: &Path, id: &str) -> TestResult {
    let res = start_container(id, project_path)
        .expect("failed to execute start command")
        .wait_with_output();
    get_result_from_output(res)
}
