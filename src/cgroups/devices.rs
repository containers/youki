use std::{
    path::Path,
};

use anyhow::Result;
use async_trait::async_trait;
use nix::unistd::Pid;
use smol::{fs::{File, OpenOptions, create_dir_all}, io::AsyncWriteExt};

use crate::{
    cgroups::Controller,
    rootfs::default_devices,
};
use oci_spec::{LinuxDeviceCgroup, LinuxDeviceType, LinuxResources};

pub struct Devices {}

#[async_trait]
impl Controller for Devices {
    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply Devices cgroup config");
        create_dir_all(&cgroup_root).await?;
        
        let mut allowed = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("devices.allow")).await?;

        let mut denied = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("devices.deny")).await?;

        for d in &linux_resources.devices {
            if d.allow {
                Self::apply_device(d, &mut allowed).await?;
            } else {
                Self::apply_device(d, &mut denied).await?;
            } 
        }

        for d in [
            default_devices().iter().map(|d| d.into()).collect(),
            Self::default_allow_devices(),
        ]
        .concat()
        {
            if d.allow {
                Self::apply_device(&d, &mut allowed).await?;
            } else {
                Self::apply_device(&d, &mut denied).await?;
            } 
        }

        allowed.sync_data().await?;
        denied.sync_data().await?;

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

impl Devices {
    async fn apply_device(device: &LinuxDeviceCgroup, file: &mut File) -> Result<()> {
        file.write_all(device.to_string().as_bytes()).await?;
        file.sync_data().await?;
        Ok(())
    }

    fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
        vec![
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: None,
                minor: None,
                access: "m".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::B,
                major: None,
                minor: None,
                access: "m".to_string(),
            },
            // /dev/console
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(5),
                minor: Some(1),
                access: "rwm".to_string(),
            },
            // /dev/pts
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(136),
                minor: None,
                access: "rwm".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(5),
                minor: Some(2),
                access: "rwm".to_string(),
            },
            // tun/tap
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(10),
                minor: Some(200),
                access: "rwm".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_device_cgroup_to_string() {
        let ldc = LinuxDeviceCgroup {
            allow: true,
            typ: LinuxDeviceType::A,
            major: None,
            minor: None,
            access: "rwm".into(),
        };
        assert_eq!(ldc.to_string(), "a *:* rwm");
        let ldc = LinuxDeviceCgroup {
            allow: true,
            typ: LinuxDeviceType::A,
            major: Some(1),
            minor: Some(9),
            access: "rwm".into(),
        };
        assert_eq!(ldc.to_string(), "a 1:9 rwm");
    }
}
