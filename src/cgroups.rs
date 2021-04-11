use std::io::Write;
use std::path::PathBuf;
use std::{
    fs::{create_dir_all, remove_dir, OpenOptions},
    path::Path,
};

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::{MountInfo, Process};

use crate::{
    rootfs::default_devices,
    spec::{LinuxDeviceCgroup, LinuxDeviceType, LinuxResources},
    utils::PathBufExt,
};

pub struct Manager {
    cgroup_path: PathBuf,
    mount_info: MountInfo,
}

impl Manager {
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let mut mount: Vec<MountInfo> = Process::myself()?
            .mountinfo()?
            .into_iter()
            .filter(|m| m.fs_type == "cgroup" && m.mount_point.ends_with("devices"))
            .collect();
        Ok(Manager {
            cgroup_path,
            mount_info: mount.pop().unwrap(),
        })
    }

    pub fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        for cgroup in Process::myself()?.cgroups()?.iter() {
            eprintln!("c: {:?}", cgroup)
        }

        let p = self
            .mount_info
            .mount_point
            // .join_absolute_path(&PathBuf::from("/user.slice"))?
            .join_absolute_path(&self.cgroup_path)?;
        create_dir_all(&p)?;
        for d in &linux_resources.devices {
            Self::apply_device(d, &p)?;
        }

        for d in default_devices().iter() {
            Self::apply_device(&d.into(), &p)?;
        }

        for d in Self::default_allow_devices().iter() {
            Self::apply_device(&d, &p)?;
        }

        let cgroups_procs = p.join("cgroup.procs");
        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(cgroups_procs)?
            .write_all(pid.to_string().as_bytes())?;

        Ok(())
    }

    fn apply_device(device: &LinuxDeviceCgroup, cgroup_root: &Path) -> Result<()> {
        let device_deny = if device.allow {
            cgroup_root.join("devices.allow")
        } else {
            cgroup_root.join("devices.deny")
        };

        let major = device
            .major
            .map(|mj| mj.to_string())
            .unwrap_or_else(|| "*".to_string());
        let minor = device
            .minor
            .map(|mi| mi.to_string())
            .unwrap_or_else(|| "*".to_string());
        let val = format! {"{} {}:{} {}", device.typ.as_str(), &major, &minor, &device.access};

        eprintln!("device_deny: {:?} val: {:?}", device_deny.display(), val);
        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(device_deny)?
            .write_all(val.as_bytes())?;
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

    pub fn remove(&self) -> Result<()> {
        let p = self
            .mount_info
            .mount_point
            .join_absolute_path(&self.cgroup_path)?;
        println!("remove_dir: {:?}", p.display());
        remove_dir(&p)?;

        Ok(())
    }
}
