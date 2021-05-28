use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct Io { }

impl Controller for Io {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &std::path::Path) -> anyhow::Result<()> {
        Ok(())
    }
}