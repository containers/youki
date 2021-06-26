use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use crate::container::Container;

#[derive(Clap, Debug)]
pub struct State {
    pub container_id: String,
}

impl State {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let root_path = fs::canonicalize(root_path)?;
        let container_root = root_path.join(&self.container_id);
        let container = Container::load(container_root)?.refresh_status()?;
        println!("{}", serde_json::to_string_pretty(&container.state)?);
        std::process::exit(0);
    }
}
