use crate::utils;
use cgroups::common;
use clap::Clap;
use std::{path::PathBuf, thread, time::Duration};

use crate::container::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};

#[derive(Clap, Debug)]
pub struct Events {
    /// Sets the stats collection interval in seconds (default: 5s)
    #[clap(long, default_value = "5")]
    pub interval: u32,
    /// Display the container stats only once
    #[clap(long)]
    pub stats: bool,
    /// Name of the container instance
    pub container_id: String,
}

impl Events {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_dir = root_path.join(&self.container_id);
        if !container_dir.exists() {
            log::debug!("{:?}", container_dir);
            bail!("{} doesn't exist.", self.container_id)
        }

        let container = Container::load(container_dir)?.refresh_status()?;
        if !container.state.status.eq(&ContainerStatus::Running) {
            bail!("{} is not in running state", self.container_id);
        }

        let cgroups_path = utils::get_cgroup_path(
            container
                .spec()?
                .linux()
                .as_ref()
                .context("no linux in spec")?
                .cgroups_path(),
            &self.container_id,
        );
        let use_systemd = container
            .systemd()
            .context("Could not determine cgroup manager")?;

        let cgroup_manager = common::create_cgroup_manager(cgroups_path, use_systemd)?;
        match self.stats {
            true => {
                let stats = cgroup_manager.stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
            }
            false => loop {
                let stats = cgroup_manager.stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
                thread::sleep(Duration::from_secs(self.interval as u64));
            },
        }

        Ok(())
    }
}
