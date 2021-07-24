//! Starts execution of the container

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Clap;
use nix::unistd;

use crate::container::{Container, ContainerStatus};
use crate::notify_socket::{NotifySocket, NOTIFY_FILE};

#[derive(Clap, Debug)]
pub struct Start {
    pub container_id: String,
}

impl Start {
    pub fn new(container_id: String) -> Self {
        return Self { container_id };
    }
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }
        let container = Container::load(container_root)?.refresh_status()?;
        if !container.can_start() {
            let err_msg = format!(
                "{} could not be started because it was {:?}",
                container.id(),
                container.status()
            );
            log::error!("{}", err_msg);
            bail!(err_msg);
        }

        unistd::chdir(container.root.as_os_str())?;

        let mut notify_socket = NotifySocket::new(&container.root.join(NOTIFY_FILE));
        notify_socket.notify_container_start()?;

        container.update_status(ContainerStatus::Running).save()?;
        Ok(())
    }
}
