use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use libcontainer::container::Container;

/// Show the container state
#[derive(Parser, Debug)]
pub struct State {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}

pub fn state(args: State, root_path: PathBuf) -> Result<()> {
    let root_path = fs::canonicalize(root_path)?;
    let container_root = root_path.join(&args.container_id);
    let container = Container::load(container_root)?;
    println!("{}", serde_json::to_string_pretty(&container.state)?);
    std::process::exit(0);
}
