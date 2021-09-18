use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use nix::unistd;
use oci_spec::runtime::LinuxCpu;
use unistd::Pid;

use crate::common::{self, ControllerOpt, CGROUP_PROCS};

use super::{util, Controller, ControllerType};

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

impl Controller for CpuSet {
    type Resource = LinuxCpu;

    fn add_task(pid: Pid, cgroup_path: &Path) -> Result<()> {
        fs::create_dir_all(cgroup_path)?;

        Self::ensure_not_empty(cgroup_path, CGROUP_CPUSET_CPUS)?;
        Self::ensure_not_empty(cgroup_path, CGROUP_CPUSET_MEMS)?;

        common::write_cgroup_file(cgroup_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }

    fn apply(controller_opt: &ControllerOpt, cgroup_path: &Path) -> Result<()> {
        log::debug!("Apply CpuSet cgroup config");

        if let Some(cpuset) = Self::needs_to_handle(controller_opt) {
            Self::apply(cgroup_path, cpuset)
                .context("failed to apply cpuset resource restrictions")?;
        }

        Ok(())
    }

    fn needs_to_handle(controller_opt: &ControllerOpt) -> Option<&Self::Resource> {
        if let Some(cpuset) = &controller_opt.resources.cpu() {
            if cpuset.cpus().is_some() || cpuset.mems().is_some() {
                return Some(cpuset);
            }
        }

        None
    }
}

impl CpuSet {
    fn apply(cgroup_path: &Path, cpuset: &LinuxCpu) -> Result<()> {
        if let Some(cpus) = &cpuset.cpus() {
            common::write_cgroup_file_str(cgroup_path.join(CGROUP_CPUSET_CPUS), cpus)?;
        }

        if let Some(mems) = &cpuset.mems() {
            common::write_cgroup_file_str(cgroup_path.join(CGROUP_CPUSET_MEMS), mems)?;
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
    use crate::test::{setup, LinuxCpuBuilder};

    #[test]
    fn test_set_cpus() {
        // arrange
        let (tmp, cpus) = setup("test_set_cpus", CGROUP_CPUSET_CPUS);
        let cpuset = LinuxCpuBuilder::new().with_cpus("1-3".to_owned()).build();

        // act
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

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
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(&mems)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUSET_MEMS));
        assert_eq!(content, "1-3");
    }
}
