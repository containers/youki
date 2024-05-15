use std::path::PathBuf;

use anyhow::Result;
use liboci_cli::State;

use crate::commands::load_container;

pub fn state(args: State, root_path: PathBuf) -> Result<()> {
    let container = load_container(root_path, &args.container_id)?;
    println!("{}", serde_json::to_string_pretty(&container.state)?);
    std::process::exit(0);
}
