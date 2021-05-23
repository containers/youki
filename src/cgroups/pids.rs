use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::Result;

use crate::{
    cgroups::Controller,
    spec::{LinuxPids, LinuxResources},
};

pub struct Pids {}

impl Controller for Pids {
    fn apply(
        linux_resources: &LinuxResources,
        cgroup_root: &std::path::Path,
        pid: nix::unistd::Pid,
    ) -> anyhow::Result<()> {
        fs::create_dir_all(cgroup_root)?;

        for pids in &linux_resources.pids {
            Self::apply(cgroup_root, pids)?
        }

        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("cgroup.procs"))?
            .write_all(pid.to_string().as_bytes())?;
        Ok(())
    }
}

impl Pids {
    fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit > 0 {
            pids.limit.to_string()
        } else {
            "max".to_string()
        };

        Self::write_file(&root_path.join("pids.max"), &limit)?;
        Ok(())
    }

    fn write_file(file_path: &Path, data: &str) -> Result<()> {
        fs::OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path)?
            .write_all(data.as_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::spec::LinuxPids;

    fn set_fixture(temp_dir: &std::path::Path, filename: &str, val: &str) -> Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(temp_dir.join(filename))?
            .write_all(val.as_bytes())?;

        Ok(())
    }

    fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_set_pids() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("pids").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPids { limit: 1000 };

        Pids::apply(&tmp, &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!(pids.limit.to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("pids").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "0").expect("set fixture for 0 pids");

        let pids = LinuxPids { limit: 0 };

        Pids::apply(&tmp, &pids).expect("apply pids");

        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!("max".to_string(), content);
    }
}
