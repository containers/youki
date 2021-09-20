//! Contains functionality of pause container command
use crate::container::Container;
use std::fs::canonicalize;
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Clap;

/// Structure to implement pause command
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
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        // populate data in a container structure from its file
        let mut container = Container::load(container_root)?;
        container.pause()
    }
}
