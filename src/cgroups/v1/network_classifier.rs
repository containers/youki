use std::io::Write;
use std::{
    fs::{create_dir_all, OpenOptions},
    path::Path,
};

use anyhow::Result;
use nix::unistd::Pid;

use crate::{cgroups::v1::Controller,};
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkClassifier {}

impl Controller for NetworkClassifier {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply NetworkClassifier cgroup config");
        create_dir_all(&cgroup_root)?;

        if let Some(network) = linux_resources.network.as_ref() {
            Self::apply(cgroup_root, network)?;

            OpenOptions::new()
                .create(false)
                .write(true)
                .truncate(true)
                .open(cgroup_root.join("cgroup.procs"))?
                .write_all(pid.to_string().as_bytes())?;
        }

        Ok(())
    }
}

impl NetworkClassifier {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(class_id) = network.class_id {
            Self::write_file(&root_path.join("net_cls.classid"), &class_id.to_string())?;
        }

        Ok(())
    }

    fn write_file(file_path: &Path, data: &str) -> Result<()> {
        OpenOptions::new()
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
    fn test_apply_network_classifier() {
        let tmp = create_temp_dir("test_apply_network_classifier")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_cls.classid", "0").expect("set fixture for classID");

        let id = 0x100001;
        let network = LinuxNetwork {
            class_id: Some(id),
            priorities: vec![],
        };

        NetworkClassifier::apply(&tmp, &network).expect("apply network classID");

        let content =
            std::fs::read_to_string(tmp.join("net_cls.classid")).expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
