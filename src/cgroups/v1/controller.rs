use std::{fs, path::Path};

use anyhow::Result;
use nix::unistd::Pid;

use oci_spec::LinuxResources;

use crate::cgroups::common::{self, CGROUP_PROCS};

pub trait Controller {
    fn add_task(pid: Pid, cgroup_path: &Path) -> Result<()> {
        fs::create_dir_all(cgroup_path)?;
        common::write_cgroup_file(cgroup_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }

    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()>;
}
