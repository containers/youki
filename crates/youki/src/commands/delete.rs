use crate::commands::load_container;
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Release any resources held by the container
#[derive(Parser, Debug)]
pub struct Delete {
    #[clap(forbid_empty_values = true, required = true)]
    container_id: String,
    /// forces deletion of the container if it is still running (using SIGKILL)
    #[clap(short, long)]
    force: bool,
}

pub fn delete(args: Delete, root_path: PathBuf) -> Result<()> {
    log::debug!("start deleting {}", args.container_id);
    let mut container = load_container(root_path, &args.container_id)?;
    container
        .delete(args.force)
        .with_context(|| format!("failed to delete container {}", args.container_id))
}
