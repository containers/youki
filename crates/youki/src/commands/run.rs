use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use libcontainer::{container::builder::ContainerBuilder, syscall::syscall::create_syscall};

/// Create a container and immediately start it
#[derive(Parser, Debug)]
pub struct Run {
    /// File to write pid of the container created
    // note that in the end, container is just another process
    #[clap(short, long)]
    pid_file: Option<PathBuf>,
    /// path to the bundle directory, containing config.json and root filesystem
    #[clap(short, long, default_value = ".")]
    bundle: PathBuf,
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(short, long)]
    console_socket: Option<PathBuf>,
    /// Pass N additional file descriptors to the container (stdio + $LISTEN_FDS + N in total)
    #[clap(long, default_value = "0")]
    preserve_fds: i32,
    /// name of the container instance to be started
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

pub fn run(args: Run, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
    let syscall = create_syscall();
    let mut container = ContainerBuilder::new(args.container_id.clone(), syscall.as_ref())
        .with_pid_file(args.pid_file.as_ref())
        .with_console_socket(args.console_socket.as_ref())
        .with_root_path(root_path)
        .with_preserved_fds(args.preserve_fds)
        .as_init(&args.bundle)
        .with_systemd(systemd_cgroup)
        .build()?;

    container
        .start()
        .with_context(|| format!("failed to start container {}", args.container_id))
}
