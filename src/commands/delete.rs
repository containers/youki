use crate::commands::{container_exists, load_container};
use anyhow::{Context, Result};
use std::path::PathBuf;

use liboci_cli::Delete;

pub fn delete(args: Delete, root_path: PathBuf) -> Result<()> {
    log::debug!("start deleting {}", args.container_id);
    if !container_exists(&root_path, &args.container_id)? && args.force {
        return Ok(());
    }

    let mut container = load_container(root_path, &args.container_id)?;
    container
        .delete(args.force)
        .with_context(|| format!("failed to delete container {}", args.container_id))
}
