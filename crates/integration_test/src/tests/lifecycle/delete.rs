use super::get_result_from_output;
use crate::utils::{delete_container};
use std::path::Path;
use test_framework::TestResult;

pub fn delete(project_path: &Path, id: &str) -> TestResult {
    let res = delete_container(id, project_path)
        .expect("failed to execute delete command")
        .wait_with_output();
    get_result_from_output(res)
}
