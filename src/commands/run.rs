use std::path::PathBuf;

use crate::commands::create::Create;
use crate::commands::start::Start;
use anyhow::Result;
use clap::Clap;
/// Create and start a container.
/// a shortcut for create followed by start.
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
    /// name of the container instance to be started
    pub container_id: String,
}

impl Run {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        Create::new(
            self.container_id.clone(),
            self.pid_file.clone(),
            self.bundle.clone(),
            self.console_socket.clone(),
        )
        .exec(root_path.clone(), systemd_cgroup)?;

        Start::new(self.container_id.clone()).exec(root_path)?;

        Ok(())
    }
}
