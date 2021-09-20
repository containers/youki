//! Contains functionality of kill container command
use std::{fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::Clap;

use crate::{container::Container, signal::ToSignal};

#[derive(Clap, Debug)]
pub struct Kill {
    #[clap(forbid_empty_values = true, required = true)]
    container_id: String,
    signal: String,
}

impl Kill {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        // resolves relative paths, symbolic links etc. and get complete path
        let root_path = fs::canonicalize(root_path)?;
        // state of container is stored in a directory named as container id inside
        // root directory given in commandline options
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }

        // load container state from json file, and check status of the container
        // it might be possible that kill is invoked on a already stopped container etc.
        let container = Container::load(container_root)?.refresh_status()?;
        let signal = self
            .signal
            .to_signal()
            .with_context(|| format!("signal {} is unknown", self.signal))?;
        container.kill(signal)
    }
}
