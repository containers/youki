use clap::Clap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::load_container;

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
        let mut container = load_container(root_path, &self.container_id)?;
        container
            .events(self.interval, self.stats)
            .with_context(|| format!("failed to get events from container {}", self.container_id))
    }
}
