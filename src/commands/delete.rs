use crate::container::Container;
use anyhow::{bail, Context, Result};
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
        // state of container is stored in a directory named as container id inside
        // root directory given in commandline options
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }
        // load container state from json file, and check status of the container
        // it might be possible that delete is invoked on a running container.
        log::debug!("load the container from {:?}", container_root);
        let mut container = Container::load(container_root)?;
        container
            .delete(self.force)
            .with_context(|| format!("failed to delete container {}", self.container_id))
    }
}
