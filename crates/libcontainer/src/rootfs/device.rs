use super::utils::to_sflag;
use crate::syscall::{syscall::create_syscall, Syscall};
use crate::utils::{self, PathBufExt};
use anyhow::{bail, Context, Result};
use nix::{
    fcntl::{open, OFlag},
    mount::MsFlags,
    sys::stat::{umask, Mode},
    unistd::{close, Gid, Uid},
};
use oci_spec::runtime::LinuxDevice;
use std::path::{Path, PathBuf};

pub struct Device {
    syscall: Box<dyn Syscall>,
}

impl Default for Device {
    fn default() -> Self {
        Self::new()
    }
}

impl Device {
    pub fn new() -> Device {
        Device {
            syscall: create_syscall(),
        }
    }

    pub fn new_with_syscall(syscall: Box<dyn Syscall>) -> Device {
        Device { syscall }
    }

    pub fn create_devices<'a, I>(&self, rootfs: &Path, devices: I, bind: bool) -> Result<()>
    where
        I: IntoIterator<Item = &'a LinuxDevice>,
    {
        let old_mode = umask(Mode::from_bits_truncate(0o000));
        devices
            .into_iter()
            .map(|dev| {
                if !dev.path().starts_with("/dev") {
                    bail!("{} is not a valid device path", dev.path().display());
                }

                if bind {
                    self.bind_dev(rootfs, dev)
                } else {
                    self.mknod_dev(rootfs, dev)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        umask(old_mode);

        Ok(())
    }

    fn bind_dev(&self, rootfs: &Path, dev: &LinuxDevice) -> Result<()> {
        let full_container_path = create_container_dev_path(rootfs, dev)
            .with_context(|| format!("could not create container path for device {dev:?}"))?;

        let fd = open(
            &full_container_path,
            OFlag::O_RDWR | OFlag::O_CREAT,
            Mode::from_bits_truncate(0o644),
        )?;
        close(fd)?;
        self.syscall.mount(
            Some(dev.path()),
            &full_container_path,
            Some("bind"),
            MsFlags::MS_BIND,
            None,
        )?;

        Ok(())
    }

    fn mknod_dev(&self, rootfs: &Path, dev: &LinuxDevice) -> Result<()> {
        fn makedev(major: i64, minor: i64) -> u64 {
            ((minor & 0xff)
                | ((major & 0xfff) << 8)
                | ((minor & !0xff) << 12)
                | ((major & !0xfff) << 32)) as u64
        }

        let full_container_path = create_container_dev_path(rootfs, dev)
            .with_context(|| format!("could not create container path for device {dev:?}"))?;

        self.syscall.mknod(
            &full_container_path,
            to_sflag(dev.typ()),
            Mode::from_bits_truncate(dev.file_mode().unwrap_or(0)),
            makedev(dev.major(), dev.minor()),
        )?;
        self.syscall.chown(
            &full_container_path,
            dev.uid().map(Uid::from_raw),
            dev.gid().map(Gid::from_raw),
        )?;

        Ok(())
    }
}

fn create_container_dev_path(rootfs: &Path, dev: &LinuxDevice) -> Result<PathBuf> {
    let relative_dev_path = dev
        .path()
        .as_relative()
        .with_context(|| format!("could not convert {:?} to relative path", dev.path()))?;
    let full_container_path = utils::secure_join(rootfs, relative_dev_path)
        .with_context(|| format!("could not join {:?} with {:?}", rootfs, dev.path()))?;

    crate::utils::create_dir_all(
        full_container_path
            .parent()
            .unwrap_or_else(|| Path::new("")),
    )?;

    Ok(full_container_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::test::{ChownArgs, MknodArgs, MountArgs, TestHelperSyscall};
    use anyhow::Result;
    use nix::{
        sys::stat::SFlag,
        unistd::{Gid, Uid},
    };
    use oci_spec::runtime::{LinuxDeviceBuilder, LinuxDeviceType};
    use std::path::PathBuf;

    #[test]
    fn test_bind_dev() -> Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let device = Device::new_with_syscall(Box::<TestHelperSyscall>::default());
        assert!(device
            .bind_dev(
                tmp_dir.path(),
                &LinuxDeviceBuilder::default()
                    .path(PathBuf::from("/null"))
                    .build()
                    .unwrap(),
            )
            .is_ok());

        let want = MountArgs {
            source: Some(PathBuf::from("/null")),
            target: tmp_dir.path().join("null"),
            fstype: Some("bind".to_string()),
            flags: MsFlags::MS_BIND,
            data: None,
        };
        let got = &device
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args()[0];
        assert_eq!(want, *got);
        Ok(())
    }

    #[test]
    fn test_mknod_dev() -> Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let device = Device::new_with_syscall(Box::<TestHelperSyscall>::default());
        assert!(device
            .mknod_dev(
                tmp_dir.path(),
                &LinuxDeviceBuilder::default()
                    .path(PathBuf::from("/null"))
                    .major(1)
                    .minor(3)
                    .typ(LinuxDeviceType::C)
                    .file_mode(0o644u32)
                    .uid(1000u32)
                    .gid(1000u32)
                    .build()
                    .unwrap(),
            )
            .is_ok());

        let want_mknod = MknodArgs {
            path: tmp_dir.path().join("null"),
            kind: SFlag::S_IFCHR,
            perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
            dev: 259,
        };
        let got_mknod = &device
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mknod_args()[0];
        assert_eq!(want_mknod, *got_mknod);

        let want_chown = ChownArgs {
            path: tmp_dir.path().join("null"),
            owner: Some(Uid::from_raw(1000)),
            group: Some(Gid::from_raw(1000)),
        };
        let got_chown = &device
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_chown_args()[0];
        assert_eq!(want_chown, *got_chown);

        Ok(())
    }

    #[test]
    fn test_create_devices() -> Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let device = Device::new_with_syscall(Box::<TestHelperSyscall>::default());

        let devices = vec![LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/null"))
            .major(1)
            .minor(3)
            .typ(LinuxDeviceType::C)
            .file_mode(0o644u32)
            .uid(1000u32)
            .gid(1000u32)
            .build()
            .unwrap()];

        assert!(device
            .create_devices(tmp_dir.path(), &devices, true)
            .is_ok());

        let want = MountArgs {
            source: Some(PathBuf::from("/dev/null")),
            target: tmp_dir.path().join("dev/null"),
            fstype: Some("bind".to_string()),
            flags: MsFlags::MS_BIND,
            data: None,
        };
        let got = &device
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args()[0];
        assert_eq!(want, *got);

        assert!(device
            .create_devices(tmp_dir.path(), &devices, false)
            .is_ok());

        let want = MknodArgs {
            path: tmp_dir.path().join("dev/null"),
            kind: SFlag::S_IFCHR,
            perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
            dev: 259,
        };
        let got = &device
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mknod_args()[0];
        assert_eq!(want, *got);

        Ok(())
    }
}
