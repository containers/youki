use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use nix::unistd::Pid;
use smol::{fs::{OpenOptions, create_dir_all}, io::AsyncWriteExt};

use crate::cgroups::Controller;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkPriority {}

#[async_trait]
impl Controller for NetworkPriority {
    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");
        create_dir_all(&cgroup_root).await?;

        if let Some(network) = linux_resources.network.as_ref() {
            Self::apply(cgroup_root, network).await?;

            let mut file = OpenOptions::new()
                .create(false)
                .write(true)
                .truncate(true)
                .open(cgroup_root.join("cgroup.procs")).await?;
            
            file.write_all(pid.to_string().as_bytes()).await?;
            file.sync_data().await?;
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
    use std::path::PathBuf;
    use std::io::Write;

    use super::*;
    use oci_spec::LinuxInterfacePriority;

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

    fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_apply_network_priorites() {
        let tmp = create_temp_dir("test_apply_network_priorites")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_prio.ifpriomap", "").expect("set fixture for priority map");

        smol::block_on(async {
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

            NetworkPriority::apply(&tmp, &network).await.expect("apply network priorities");

            let content =
                std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
            assert_eq!(priorities_string.trim(), content);
        });
    }
}
