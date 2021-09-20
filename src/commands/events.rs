use clap::Clap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::container::Container;

#[derive(Clap, Debug)]
pub struct Events {
    /// Sets the stats collection interval in seconds (default: 5s)
    #[clap(long, default_value = "5")]
    pub interval: u32,
    /// Display the container stats only once
    #[clap(long)]
    pub stats: bool,
    /// Name of the container instance
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

impl Events {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_dir = root_path.join(&self.container_id);
        if !container_dir.exists() {
            log::debug!("{:?}", container_dir);
            bail!("{} doesn't exist.", self.container_id)
        }

        let mut container = Container::load(container_dir)?;
        container
            .events(self.interval, self.stats)
            .with_context(|| format!("failed to get events from container {}", self.container_id))
    }
}
