//! Contains functionality of pause container command
use crate::commands::load_container;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Clap;

/// Suspend the processes within the container
#[derive(Clap, Debug)]
pub struct Pause {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

// Pausing a container indicates suspending all processes in given container
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
impl Pause {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        log::debug!("start pausing container {}", self.container_id);
        let mut container = load_container(root_path, &self.container_id)?;
        container
            .pause()
            .with_context(|| format!("failed to pause container {}", self.container_id))
    }
}
