use super::get_result_from_output;
use crate::utils::delete_container;
use anyhow::Result;
use std::path::Path;

pub fn delete(project_path: &Path, id: &str) -> Result<()> {
    let res = delete_container(id, project_path)
        .expect("failed to execute delete command")
        .wait_with_output();
    get_result_from_output(res)
}
