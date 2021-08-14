use std::{fs, path::Path};

use anyhow::{bail, Result};
use async_trait::async_trait;
use nix::unistd;
use oci_spec::{LinuxCpu, LinuxResources};
use unistd::Pid;
use rio::Rio;

use crate::cgroups::common::{self, CGROUP_PROCS};

use super::{util, Controller, ControllerType};

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

#[async_trait]
impl Controller for CpuSet {
    type Resource = LinuxCpu;

    fn add_task(pid: Pid, cgroup_path: &Path) -> Result<()> {
        fs::create_dir_all(cgroup_path)?;

        Self::ensure_not_empty(cgroup_path, CGROUP_CPUSET_CPUS)?;
        Self::ensure_not_empty(cgroup_path, CGROUP_CPUSET_MEMS)?;

        common::write_cgroup_file(cgroup_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        log::debug!("Apply CpuSet cgroup config");

        if let Some(cpuset) = Self::needs_to_handle(linux_resources) {
            Self::apply(ring, cgroup_path, cpuset).await?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(cpuset) = &linux_resources.cpu {
            if cpuset.cpus.is_some() || cpuset.mems.is_some() {
                return Some(cpuset);
            }
        }

        None
    }
}

impl CpuSet {
    async fn apply(ring: &Rio, cgroup_path: &Path, cpuset: &LinuxCpu) -> Result<()> {
        if let Some(cpus) = &cpuset.cpus {
            let cpus_file = common::open_cgroup_file(cgroup_path.join(CGROUP_CPUSET_CPUS))?;
            common::async_write_cgroup_file_str(ring, &cpus_file,  cpus).await?;
        }

        if let Some(mems) = &cpuset.mems {
            let mems_file = common::open_cgroup_file(cgroup_path.join(CGROUP_CPUSET_MEMS))?;
            common::async_write_cgroup_file_str(ring, &mems_file, mems).await?;
        }

        Ok(())
    }

    // if a task is moved into the cgroup and a value has not been set for cpus and mems
    // Errno 28 (no space left on device) will be returned. Therefore we set the value from the parent if required.
    fn ensure_not_empty(cgroup_path: &Path, interface_file: &str) -> Result<()> {
        let mut current = util::get_subsystem_mount_point(&ControllerType::CpuSet)?;
        let relative_cgroup_path = cgroup_path.strip_prefix(&current)?;

        for component in relative_cgroup_path.components() {
            let parent_value = fs::read_to_string(current.join(interface_file))?;
            if parent_value.trim().is_empty() {
                bail!("cpuset parent value is empty")
            }

            current.push(component);
            let child_path = current.join(interface_file);
            let child_value = fs::read_to_string(&child_path)?;
            // the file can contain a newline character. Need to trim it away,
            // otherwise it is not considered empty and value will not be written
            if child_value.trim().is_empty() {
                common::write_cgroup_file_str(&child_path, &parent_value)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cgroups::test::{setup, LinuxCpuBuilder, aw};

    #[test]
    fn test_set_cpus() {
        // arrange
        let (tmp, cpus) = setup("test_set_cpus", CGROUP_CPUSET_CPUS);
        let cpuset = LinuxCpuBuilder::new().with_cpus("1-3".to_owned()).build();

        // act
        let ring = rio::new().expect("start io_uring");
        aw!(CpuSet::apply(&ring, &tmp, &cpuset)).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(&cpus)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUSET_CPUS));
        assert_eq!(content, "1-3");
    }

    #[test]
    fn test_set_mems() {
        // arrange
        let (tmp, mems) = setup("test_set_mems", CGROUP_CPUSET_MEMS);
        let cpuset = LinuxCpuBuilder::new().with_mems("1-3".to_owned()).build();

        // act
        let ring = rio::new().expect("start io_uring");
        aw!(CpuSet::apply(&ring, &tmp, &cpuset)).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(&mems)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUSET_MEMS));
        assert_eq!(content, "1-3");
    }
}
