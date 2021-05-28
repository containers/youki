use std::path::Path;
use anyhow::Result;

use oci_spec::LinuxResources;

pub trait Controller {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()>;
}