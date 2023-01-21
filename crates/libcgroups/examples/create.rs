use anyhow::Result;
use libcgroups::common::{create_cgroup_manager, DEFAULT_CGROUP_ROOT};
use nix::libc::pid_t;
use nix::unistd::Pid;
use std::path::Path;
use std::process::Command;

fn main() -> Result<()> {
    // Create cgroup manager
    let manager = create_cgroup_manager(Path::new(DEFAULT_CGROUP_ROOT), false, "example-cgroup")?;

    // Run process
    let cmd = Command::new("sh")
        .args(["-c", "sleep 100"])
        .spawn()
        .expect("spawning sleep");

    // Add the new process to the cgroup
    manager.add_task(Pid::from_raw(cmd.id() as pid_t))?;

    Ok(())
}
