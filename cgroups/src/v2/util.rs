use std::path::PathBuf;

use anyhow::{anyhow, Result};
use procfs::process::Process;

pub fn get_unified_mount_point() -> Result<PathBuf> {
    Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup2")
        .map(|m| m.mount_point)
        .ok_or_else(|| anyhow!("could not find mountpoint for unified"))
}
