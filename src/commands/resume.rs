//! Contains functionality of resume container command
use std::fs::canonicalize;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Clap;

use crate::container::Container;

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
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        let mut container = Container::load(container_root)?;
        container
            .resume()
            .with_context(|| format!("failed to resume container {}", self.container_id))
    }
}
