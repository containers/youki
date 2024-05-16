use std::path::PathBuf;

use anyhow::{Context, Result};
use liboci_cli::Events;

use crate::commands::load_container;

pub fn events(args: Events, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .events(args.interval, args.stats)
        .with_context(|| format!("failed to get events from container {}", args.container_id))
}
