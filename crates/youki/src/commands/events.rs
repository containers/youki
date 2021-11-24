use clap::Parser;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::load_container;

/// Show resource statistics for the container
#[derive(Parser, Debug)]
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

pub fn events(args: Events, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .events(args.interval, args.stats)
        .with_context(|| format!("failed to get events from container {}", args.container_id))
}
