//! Contains functionality of pause container command
use std::path::PathBuf;

use anyhow::{Context, Result};
use liboci_cli::Pause;

use crate::commands::load_container;

// Pausing a container indicates suspending all processes in given container
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
pub fn pause(args: Pause, root_path: PathBuf) -> Result<()> {
    tracing::debug!("start pausing container {}", args.container_id);
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .pause()
        .with_context(|| format!("failed to pause container {}", args.container_id))
}
