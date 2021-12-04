use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

use libcontainer::container::Container;

pub mod completion;
pub mod create;
pub mod delete;
pub mod events;
pub mod exec;
pub mod info;
pub mod kill;
pub mod list;
pub mod pause;
pub mod ps;
pub mod resume;
pub mod run;
pub mod spec_json;
pub mod start;
pub mod state;

fn load_container<P: AsRef<Path>>(root_path: P, container_id: &str) -> Result<Container> {
    // resolves relative paths, symbolic links etc. and get complete path
    let root_path = fs::canonicalize(&root_path)
        .with_context(|| format!("failed to canonicalize {}", root_path.as_ref().display()))?;
    // the state of the container is stored in a directory named after the container id
    let container_root = root_path.join(container_id);
    if !container_root.exists() {
        bail!("{} does not exist.", container_id)
    }

    Container::load(container_root)
        .with_context(|| format!("could not load state for container {}", container_id))
}
