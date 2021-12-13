use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

use libcgroups::common::CgroupManager;
use libcontainer::container::Container;
use libcontainer::utils;

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
pub mod update;

fn load_container<P: AsRef<Path>>(root_path: P, container_id: &str) -> Result<Container> {
    // resolves relative paths, symbolic links etc. and get complete path
    let root_path = fs::canonicalize(&root_path)
        .with_context(|| format!("failed to canonicalize {}", root_path.as_ref().display()))?;
    // the state of the container is stored in a directory named after the container id
    let container_root = root_path.join(container_id);
    if !container_root.exists() {
        bail!("container {} does not exist.", container_id)
    }

    Container::load(container_root)
        .with_context(|| format!("could not load state for container {}", container_id))
}

fn create_cgroup_manager<P: AsRef<Path>>(
    root_path: P,
    container_id: &str,
) -> Result<Box<dyn CgroupManager>> {
    let container = load_container(root_path, &container_id)?;
    let config_absolute_path = container.root.join("config.json");
    log::debug!("load spec from {:?}", config_absolute_path);
    let spec = oci_spec::runtime::Spec::load(config_absolute_path)?;
    log::debug!("spec: {:?}", spec);
    let cgroups_path = utils::get_cgroup_path(
        spec.linux()
            .as_ref()
            .context("no linux in spec")?
            .cgroups_path(),
        container.id(),
    );
    let systemd_cgroup = container
        .systemd()
        .context("could not determine cgroup manager")?;

    Ok(libcgroups::common::create_cgroup_manager(
        cgroups_path,
        systemd_cgroup,
        container.id(),
    )?)
}
