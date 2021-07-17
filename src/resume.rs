//! Contains functionality of resume container command
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
pub struct Resume {
    pub container_id: String,
}

impl Resume {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        log::debug!("start resuming container {}", self.container_id);
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        let container = Container::load(container_root)?.refresh_status()?;
        if !container.can_resume() {
            bail!(
                "{} could not be resumed because it was {:?}",
                self.container_id,
                container.status()
            );
        }

        let spec = container.spec()?;
        let cgroups_path =
            utils::get_cgroup_path(&spec.linux.unwrap().cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
        cmanager.freeze(FreezerState::Thawed)?;

        log::debug!("saving running status");
        container.update_status(ContainerStatus::Running).save()?;

        log::debug!("container {} resumed", self.container_id);
        Ok(())
    }
}
