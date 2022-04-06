//! Starts execution of the container

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::load_container;

use liboci_cli::Start;

pub fn start(args: Start, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .start()
        .with_context(|| format!("failed to start container {}", args.container_id))
}
