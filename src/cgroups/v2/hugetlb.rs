use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct HugeTlb {}

impl Controller for HugeTlb {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &std::path::Path) -> anyhow::Result<()> {
        Ok(())
    }
}