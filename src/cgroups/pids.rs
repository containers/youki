use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use smol::{fs::{OpenOptions, create_dir_all}, io::AsyncWriteExt};

use crate::{
    cgroups::Controller,
};
use oci_spec::{LinuxPids, LinuxResources};

pub struct Pids {}

#[async_trait]
impl Controller for Pids {
    async fn apply(
        linux_resources: &LinuxResources,
        cgroup_root: &std::path::Path,
        pid: nix::unistd::Pid,
    ) -> anyhow::Result<()> {
        create_dir_all(cgroup_root).await?;

        for pids in &linux_resources.pids {
            Self::apply(cgroup_root, pids).await?
        }

        let mut file = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("cgroup.procs")).await?;
        
        file.write_all(pid.to_string().as_bytes()).await?;
        file.sync_data().await?;
        Ok(())
    }
}

impl Pids {
    async fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit > 0 {
            pids.limit.to_string()
        } else {
            "max".to_string()
        };

        Self::write_file(&root_path.join("pids.max"), &limit).await?;
        Ok(())
    }

    async fn write_file(file_path: &Path, data: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path).await?;
        
        file.write_all(data.as_bytes()).await?;
        file.sync_data().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::LinuxPids;
    use std::io::Write;

    fn set_fixture(temp_dir: &std::path::Path, filename: &str, val: &str) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(temp_dir.join(filename))?;

        file.write_all(val.as_bytes())?;
        file.sync_data()?;

        Ok(())
    }

    fn create_temp_dir(test_name: &str) -> Result<std::path::PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_set_pids() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "1000").expect("Set fixture for 1000 pids");

        smol::block_on(async {
            let pids = LinuxPids { limit: 1000 };

            Pids::apply(&tmp, &pids).await.expect("apply pids");
            let content =
                std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
            assert_eq!(pids.limit.to_string(), content);
        });
    }

    #[test]
    fn test_set_pids_max() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "0").expect("set fixture for 0 pids");


        smol::block_on(async {
            let pids = LinuxPids { limit: 0 };

            Pids::apply(&tmp, &pids).await.expect("apply pids");

            let content =
                std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
            assert_eq!("max".to_string(), content);
        });
    }
}
