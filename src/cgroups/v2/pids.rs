use anyhow::Result;

use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct Pids {}

impl Controller for Pids {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &std::path::Path) -> Result<()> {
        Ok(())
    }
}