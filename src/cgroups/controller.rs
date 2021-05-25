use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use nix::unistd::Pid;

use oci_spec::LinuxResources;

#[async_trait]
pub trait Controller {
    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()>;
}
