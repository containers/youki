//! Handles the creation of a new container
use anyhow::Result;
use clap::Clap;
use std::path::PathBuf;

use libcontainer::{container::builder::ContainerBuilder, syscall::syscall::create_syscall};

/// Create a container
#[derive(Clap, Debug)]
pub struct Create {
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

// One thing to note is that in the end, container is just another process in Linux
// it has specific/different control group, namespace, using which program executing in it
// can be given impression that is is running on a complete system, but on the system which
// it is running, it is just another process, and has attributes such as pid, file descriptors, etc.
// associated with it like any other process.
impl Create {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        let syscall = create_syscall();
        ContainerBuilder::new(self.container_id.clone(), syscall.as_ref())
            .with_pid_file(self.pid_file.as_ref())
            .with_console_socket(self.console_socket.as_ref())
            .with_root_path(root_path)
            .with_preserved_fds(self.preserve_fds)
            .as_init(&self.bundle)
            .with_systemd(systemd_cgroup)
            .build()?;

        Ok(())
    }
}
