use std::path::PathBuf;

use crate::container::builder::ContainerBuilder;
use crate::syscall::syscall::create_syscall;
use anyhow::{Context, Result};
use clap::Clap;

/// Create a container and immediately start it
#[derive(Clap, Debug)]
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
    #[clap(required = true)]
    pub container_id: String,
}

impl Run {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        let syscall = create_syscall();
        let mut container = ContainerBuilder::new(self.container_id.clone(), syscall.as_ref())
            .with_pid_file(self.pid_file.as_ref())
            .with_console_socket(self.console_socket.as_ref())
            .with_root_path(root_path)
            .with_preserved_fds(self.preserve_fds)
            .as_init(&self.bundle)
            .with_systemd(systemd_cgroup)
            .build()?;

        container
            .start()
            .with_context(|| format!("failed to start container {}", self.container_id))
    }
}
