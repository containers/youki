use std::{thread, time::Duration};

use crate::utils;

use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};

impl Container {
    /// Displays container events
    ///
    /// # Example
    ///
    /// ```no_run
    /// use youki::container::builder::ContainerBuilder;
    /// use youki::syscall::syscall::create_syscall;;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.events(5000, false)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn events(&mut self, interval: u32, stats: bool) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;
        if !self.state.status.eq(&ContainerStatus::Running) {
            bail!("{} is not in running state", self.id());
        }

        let cgroups_path = utils::get_cgroup_path(
            self.spec()?
                .linux()
                .as_ref()
                .context("no linux in spec")?
                .cgroups_path(),
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
