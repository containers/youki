use std::path::Path;

use anyhow::Result;
use nix::unistd::Pid;

use oci_spec::LinuxResources;

pub trait Controller {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()>;
}
