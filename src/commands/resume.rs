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

/// Structure to implement resume command
#[derive(Clap, Debug)]
pub struct Resume {
    pub container_id: String,
}

// Resuming a container indicates resuming all processes in given container from paused state
// This uses Freezer cgroup to suspend and resume processes
// For more information see :
// https://man7.org/linux/man-pages/man7/cgroups.7.html
// https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt
impl Resume {
    pub fn exec(&self, root_path: PathBuf, systemd_cgroup: bool) -> Result<()> {
        log::debug!("start resuming container {}", self.container_id);
        let root_path = canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        let container = Container::load(container_root)?.refresh_status()?;
        // check if container can be resumed :
        // for example, a running process cannot be resumed
        if !container.can_resume() {
            bail!(
                "{} could not be resumed because it was {:?}",
                self.container_id,
                container.status()
            );
        }

        let spec = container.spec()?;
        // get cgroup path defined in spec
        let path_in_spec = match spec.linux {
            Some(linux) => linux.cgroups_path,
            None => None,
        };
        let cgroups_path = utils::get_cgroup_path(&path_in_spec, &self.container_id);
        // create cgroup manager structure from the config at the path
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
        // resume the frozen container
        cmanager.freeze(FreezerState::Thawed)?;

        log::debug!("saving running status");
        container.update_status(ContainerStatus::Running).save()?;

        log::debug!("container {} resumed", self.container_id);
        Ok(())
    }
}
