#![allow(unused_imports)]

//! Handles the creation of a new container
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Result};
use clap::Clap;
use nix::sched;
use nix::unistd;
use nix::unistd::{Gid, Uid};

use crate::container::builder::ContainerBuilder;
use crate::container::{Container, ContainerStatus};
use crate::namespaces::Namespaces;
use crate::notify_socket::NotifyListener;
use crate::process::{fork, Process};
use crate::rootfs;
use crate::rootless::{lookup_map_binaries, should_use_rootless, Rootless};
use crate::stdio::FileDescriptor;
use crate::tty;
use crate::utils;
use crate::{capabilities, command::Syscall};
use crate::{cgroups, rootless};

/// This is the main structure which stores various commandline options given by
/// high-level container runtime
#[derive(Clap, Debug)]
pub struct Create {
    /// File to write pid of the container created
    // note that in the end, container is just another process
    #[clap(short, long)]
    pid_file: Option<String>,
    /// path to the bundle directory, containing config.json and root filesystem
    #[clap(short, long, default_value = ".")]
    bundle: PathBuf,
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(short, long)]
    console_socket: Option<PathBuf>,
    /// name of the container instance to be started
    pub container_id: String,
}

// One thing to note is that in the end, container is just another process in Linux
// it has specific/different control group, namespace, using which program executing in it
// can be given impression that is is running on a complete system, but on the system which
// it is running, it is just another process, and has attributes such as pid, file descriptors, etc.
// associated with it like any other process.
impl Create {
    /// Starts a new container process
    pub fn exec(
        &self,
        root_path: PathBuf,
        systemd_cgroup: bool,
        command: impl Syscall,
    ) -> Result<()> {

        let mut builder = ContainerBuilder::new_init(self.container_id.to_owned(), self.bundle.clone())?;
        if let Some(pid_file) = &self.pid_file {
            builder = builder.with_pid_file(pid_file);
        }

        if let Some(console_socket) = &self.console_socket {
            builder = builder.with_console_socket(console_socket);
        }
 
        builder
        .with_root_path(root_path)
        .with_systemd(systemd_cgroup)
        .build()?;

        Ok(())
    }
}


