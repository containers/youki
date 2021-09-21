//! Starts execution of the container

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Clap;

use crate::container::Container;

#[derive(Clap, Debug)]
pub struct Start {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

impl Start {
    pub fn new(container_id: String) -> Self {
        Self { container_id }
    }

    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }
        let mut container = Container::load(container_root)?.refresh_status()?;
        container
            .start()
            .with_context(|| format!("failed to start container {}", self.container_id))
    }
}
