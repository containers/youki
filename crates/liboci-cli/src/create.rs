//! Handles the creation of a new container
use std::path::PathBuf;

use clap::Parser;

/// Create a container
/// Reference: https://github.com/opencontainers/runc/blob/main/man/runc-create.8.md
#[derive(Parser, Debug)]
pub struct Create {
    /// Path to the bundle directory, containing config.json and root filesystem
    #[clap(short, long, default_value = ".")]
    pub bundle: PathBuf,
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(short, long)]
    pub console_socket: Option<PathBuf>,
    /// File to write pid of the container created
    // note that in the end, container is just another process
    #[clap(short, long)]
    pub pid_file: Option<PathBuf>,
    /// Do not use pivot rool to jail process inside rootfs
    #[clap(long)]
    pub no_pivot: bool,
    /// Do not create a new session keyring for the container.
    #[clap(long)]
    pub no_new_keyring: bool,
    /// Pass N additional file descriptors to the container (stdio + $LISTEN_FDS + N in total)
    #[clap(long, default_value = "0")]
    pub preserve_fds: i32,

    /// Name of the container instance to be started
    #[clap(value_parser = clap::builder::NonEmptyStringValueParser::new(), required = true)]
    pub container_id: String,
}
