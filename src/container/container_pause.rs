use crate::utils;

use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};
use cgroups::common::FreezerState;

impl Container {
    pub fn pause(&mut self) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;

        if !self.can_pause() {
            bail!(
                "{} could not be paused because it was {:?}",
                self.id(),
                self.status()
            );
        }

        let spec = self.spec()?;
        let cgroups_path = utils::get_cgroup_path(
            &spec.linux.context("no linux in spec")?.cgroups_path,
            &self.id(),
        );

        let use_systemd = self
            .systemd()
            .context("container state does not contain cgroup manager")?;
        let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, use_systemd)?;
        cmanager.freeze(FreezerState::Frozen)?;

        log::debug!("saving paused status");
        self.update_status(ContainerStatus::Paused).save()?;

        log::debug!("container {} paused", self.id());
        Ok(())
    }
}
