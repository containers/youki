//! Contains functionality of pause container command
use std::fs::canonicalize;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Clap;

use crate::container::Container;
use crate::container::ContainerStatus;
use crate::utils;
use cgroups;
use cgroups::common::FreezerState;

/// Structure to implement pause command
#[derive(Clap, Debug)]
pub struct Pause {
    pub container_id: String,
}

// Pausing a container indicates suspending all processes in given container
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
impl Pause {
    /// Suspend the running container
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        log::debug!("start pausing container {}", self.container_id);
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        // populate data in a container structure from its file
        let container = Container::load(container_root)?.refresh_status()?;
        // check if a container is pauseable :
        // for example, a stopped container cannot be paused
        if !container.can_pause() {
            bail!(
                "{} could not be paused because it was {:?}",
                self.container_id,
                container.status()
            );
        }

        let spec = container.spec()?;
        let cgroups_path = utils::get_cgroup_path(
            &spec.linux.context("no linux in spec")?.cgroups_path,
            &self.container_id,
        );
        // create cgroup manager structure from the config at the path
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
        // freeze the container
        cmanager.freeze(FreezerState::Frozen)?;

        log::debug!("saving paused status");
        container.update_status(ContainerStatus::Paused).save()?;

        log::debug!("container {} paused", self.container_id);
        Ok(())
    }
}
