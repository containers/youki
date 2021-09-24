//! Contains functionality of kill container command
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Clap;

use crate::{commands::load_container, signal::ToSignal};

#[derive(Clap, Debug)]
pub struct Kill {
    #[clap(forbid_empty_values = true, required = true)]
    container_id: String,
    signal: String,
}

impl Kill {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let mut container = load_container(root_path, &self.container_id)?;
        let signal = self
            .signal
            .to_signal()
            .with_context(|| format!("signal {} is unknown", self.signal))?;
        container.kill(signal)
    }
}
