use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use nix::unistd::Pid;
use smol::{fs::{OpenOptions, create_dir_all}, io::AsyncWriteExt};

use crate::{
    cgroups::Controller,
    spec::{LinuxInterfacePriority, LinuxNetwork, LinuxResources},
};

impl ToString for LinuxInterfacePriority {
    fn to_string(&self) -> String {
        format!("{} {}\n", self.name, self.priority)
    }
}

pub struct NetworkPriority {}

#[async_trait]
impl Controller for NetworkPriority {
    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");
        create_dir_all(&cgroup_root).await?;

        if let Some(network) = linux_resources.network.as_ref() {
            Self::apply(cgroup_root, network).await?;

            OpenOptions::new()
                .create(false)
                .write(true)
                .truncate(true)
                .open(cgroup_root.join("cgroup.procs")).await?
                .write_all(pid.to_string().as_bytes()).await?;
        }

        Ok(())
    }
}

impl NetworkPriority {
    async fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        let priorities: String = network.priorities.iter().map(|p| p.to_string()).collect();
        Self::write_file(&root_path.join("net_prio.ifpriomap"), &priorities.trim()).await?;

        Ok(())
    }

    async fn write_file(file_path: &Path, data: &str) -> Result<()> {
        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path).await?
            .write_all(data.as_bytes()).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

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
    fn test_apply_network_priorites() {
        let tmp = create_temp_dir("test_apply_network_priorites")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_prio.ifpriomap", "").expect("set fixture for priority map");
        let priorities = vec![
            LinuxInterfacePriority {
                name: "a".to_owned(),
                priority: 1,
            },
            LinuxInterfacePriority {
                name: "b".to_owned(),
                priority: 2,
            },
        ];
        let priorities_string = priorities
            .clone()
            .iter()
            .map(|p| p.to_string())
            .collect::<String>();
        let network = LinuxNetwork {
            class_id: None,
            priorities,
        };

        NetworkPriority::apply(&tmp, &network).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
