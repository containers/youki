use std::path::Path;

use anyhow::Result;

use super::get_result_from_output;
use crate::utils::kill_container;

pub fn kill(project_path: &Path, id: &str) -> Result<()> {
    let res = kill_container(id, project_path)
        .expect("failed to execute kill command")
        .wait_with_output();
    get_result_from_output(res)
}
