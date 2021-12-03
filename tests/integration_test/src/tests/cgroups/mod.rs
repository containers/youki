use std::path::PathBuf;

use anyhow::{Context, Result};
use procfs::process::Process;
use std::fs;

pub mod cpus;
pub mod memory;
pub mod pids;

pub fn cleanup() -> Result<()> {
    for subsystem in list_subsystem_mount_points()? {
        let runtime_test = subsystem.join("runtime-test");
        if runtime_test.exists() {
            fs::remove_dir(&runtime_test)
                .with_context(|| format!("failed to delete {:?}", runtime_test))?;
        }
    }

    Ok(())
}

pub fn list_subsystem_mount_points() -> Result<Vec<PathBuf>> {
    Ok(Process::myself()
        .context("failed to get self")?
        .mountinfo()
        .context("failed to get mountinfo")?
        .into_iter()
        .filter_map(|m| {
            if m.fs_type == "cgroup" {
                Some(m.mount_point)
            } else {
                None
            }
        })
        .collect())
}
