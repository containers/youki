//! Contains functionality of resume container command
use std::path::PathBuf;

use anyhow::{Context, Result};
use liboci_cli::Resume;

use crate::commands::load_container;

// Resuming a container indicates resuming all processes in given container from paused state
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
pub fn resume(args: Resume, root_path: PathBuf) -> Result<()> {
    tracing::debug!("start resuming container {}", args.container_id);
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .resume()
        .with_context(|| format!("failed to resume container {}", args.container_id))
}
