//! Starts execution of the container

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::commands::load_container;

/// Start a previously created container
#[derive(Parser, Debug)]
pub struct Start {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

impl Start {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let mut container = load_container(root_path, &self.container_id)?;
        container
            .start()
            .with_context(|| format!("failed to start container {}", self.container_id))
    }
}
