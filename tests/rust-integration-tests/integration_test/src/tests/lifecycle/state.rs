use crate::utils::get_state;
use anyhow::{bail, Result};
use std::path::Path;

pub fn state(project_path: &Path, id: &str) -> Result<()> {
    match get_state(id, project_path) {
        Ok((stdout, stderr)) => {
            if stderr.contains("Error") || stderr.contains("error") {
                bail!("Error :\nstdout : {}\nstderr : {}", stdout, stderr)
            } else {
                // confirm that the status is stopped, as this is executed after the kill command
                if !(stdout.contains(&format!(r#""id": "{id}""#))
                    && stdout.contains(r#""status": "stopped""#))
                {
                    bail!("Expected state stopped, got : {}", stdout)
                } else {
                    Ok(())
                }
            }
        }
        Err(e) => Err(e.context("failed to get container state")),
    }
}
