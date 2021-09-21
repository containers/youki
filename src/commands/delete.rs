use crate::commands::load_container;
use anyhow::{Context, Result};
use clap::Clap;
use std::path::PathBuf;

#[derive(Clap, Debug)]
pub struct Delete {
    #[clap(forbid_empty_values = true, required = true)]
    container_id: String,
    /// forces deletion of the container if it is still running (using SIGKILL)
    #[clap(short, long)]
    force: bool,
}

impl Delete {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        log::debug!("start deleting {}", self.container_id);
        let mut container = load_container(root_path, self.container_id.as_str())?;
        container
            .delete(self.force)
            .with_context(|| format!("failed to delete container {}", self.container_id))
    }
}
