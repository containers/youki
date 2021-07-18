//! Contains functionality of pause container command
use std::fs::canonicalize;
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Clap;

use crate::cgroups;
use crate::container::Container;
use crate::container::ContainerStatus;
use crate::utils;
use oci_spec::FreezerState;

#[derive(Clap, Debug)]
pub struct Pause {
    pub container_id: String,
}

impl Pause {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        log::debug!("start pausing container {}", self.container_id);
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        let container = Container::load(container_root)?.refresh_status()?;
        if !container.can_pause() {
            bail!(
                "{} could not be paused because it was {:?}",
                self.container_id,
                container.status()
            );
        }

        let spec = container.spec()?;
        let cgroups_path =
            utils::get_cgroup_path(&spec.linux.unwrap().cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
        cmanager.freeze(FreezerState::Frozen)?;

        log::debug!("saving paused status");
        container.update_status(ContainerStatus::Paused).save()?;

        log::debug!("container {} paused", self.container_id);
        Ok(())
    }
}
