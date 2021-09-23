use std::{thread, time::Duration};

use crate::utils;

use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};

impl Container {
    pub fn events(&mut self, interval: u32, stats: bool) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;
        if !self.state.status.eq(&ContainerStatus::Running) {
            bail!("{} is not in running state", self.id());
        }

        let cgroups_path = utils::get_cgroup_path(
            &self.spec()?.linux.context("no linux in spec")?.cgroups_path,
            self.id(),
        );
        let use_systemd = self
            .systemd()
            .context("Could not determine cgroup manager")?;

        let cgroup_manager = cgroups::common::create_cgroup_manager(cgroups_path, use_systemd)?;
        match stats {
            true => {
                let stats = cgroup_manager.stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
            }
            false => loop {
                let stats = cgroup_manager.stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
                thread::sleep(Duration::from_secs(interval as u64));
            },
        }

        Ok(())
    }
}
