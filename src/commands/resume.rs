//! Contains functionality of resume container command
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Clap;

use crate::commands::load_container;

/// Structure to implement resume command
#[derive(Clap, Debug)]
pub struct Resume {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

// Resuming a container indicates resuming all processes in given container from paused state
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
impl Resume {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        log::debug!("start resuming container {}", self.container_id);
        let mut container = load_container(root_path, &self.container_id)?;
        container
            .resume()
            .with_context(|| format!("failed to resume container {}", self.container_id))
    }
}
