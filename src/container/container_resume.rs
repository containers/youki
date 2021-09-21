use crate::utils;

use super::{Container, ContainerStatus};

use anyhow::{bail, Context, Result};
use cgroups::common::FreezerState;

impl Container {
    pub fn resume(&mut self) -> Result<()> {
        self.refresh_status()
            .with_context(|| format!("failed to refresh container status"))?;
        // check if container can be resumed :
        // for example, a running process cannot be resumed
        if !self.can_resume() {
            bail!(
                "{} could not be resumed because it was {:?}",
                self.id(),
                self.status()
            );
        }

        let spec = self.spec()?;
        let cgroups_path = utils::get_cgroup_path(
            &spec.linux.context("no linux in spec")?.cgroups_path,
            &self.id(),
        );

        // create cgroup manager structure from the config at the path
        let use_systemd = self
            .systemd()
            .context("container state does not contain cgroup manager")?;
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, use_systemd)?;
        // resume the frozen container
        cmanager.freeze(FreezerState::Thawed)?;

        log::debug!("saving running status");
        self.update_status(ContainerStatus::Running).save()?;

        log::debug!("container {} resumed", self.id());
        Ok(())
    }
}
