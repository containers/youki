use std::path::PathBuf;

use anyhow::{Context, Result};
use libcontainer::{container::builder::ContainerBuilder, syscall::syscall::create_syscall};
use liboci_cli::Run;

pub fn run(args: Run, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
    let syscall = create_syscall();
    let mut container = ContainerBuilder::new(args.container_id.clone(), syscall.as_ref())
        .with_pid_file(args.pid_file.as_ref())?
        .with_console_socket(args.console_socket.as_ref())
        .with_root_path(root_path)?
        .with_preserved_fds(args.preserve_fds)
        .as_init(&args.bundle)
        .with_systemd(systemd_cgroup)
        .build()?;

    container
        .start()
        .with_context(|| format!("failed to start container {}", args.container_id))
}
