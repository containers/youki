use std::{fs, path::PathBuf};

use anyhow::{bail, Result};
use clap::Clap;
use nix::sys::signal as nix_signal;

use crate::{
    container::{Container, ContainerStatus},
    signal::ToSignal,
};

#[derive(Clap, Debug)]
pub struct Kill {
    container_id: String,
    signal: String,
}

impl Kill {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        // resolves relative paths, symbolic links etc. and get complete path
        let root_path = fs::canonicalize(root_path)?;
        // state of container is stored in a directory named as container id inside
        // root directory given in commandline options
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        // load container state from json file, and check status of the container
        // it might be possible that kill is invoked on a already stopped container etc.
        let container = Container::load(container_root)?.refresh_status()?;
        if container.can_kill() {
            let sig = self.signal.to_signal()?;
            log::debug!("kill signal {} to {}", sig, container.pid().unwrap());
            nix_signal::kill(container.pid().unwrap(), sig)?;
            container.update_status(ContainerStatus::Stopped).save()?;
            std::process::exit(0)
        } else {
            bail!(
                "{} could not be killed because it was {:?}",
                container.id(),
                container.status()
            )
        }
    }
}
