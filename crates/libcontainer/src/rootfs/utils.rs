use crate::syscall::linux::{self, MountAttrOption};
use nix::{mount::MsFlags, sys::stat::SFlag};
use oci_spec::runtime::{LinuxDevice, LinuxDeviceBuilder, LinuxDeviceType, Mount};
use std::{path::PathBuf, str::FromStr};

use super::mount::MountError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountOptionConfig {
    /// Mount Flags.
    pub flags: MsFlags,

    /// Mount data applied to the mount.
    pub data: String,

    /// RecAttr represents mount properties to be applied recursively.
    pub rec_attr: Option<linux::MountAttr>,
}

pub fn default_devices() -> Vec<LinuxDevice> {
    vec![
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/null"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(3)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/zero"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(5)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/full"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(7)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/tty"))
            .typ(LinuxDeviceType::C)
            .major(5)
            .minor(0)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/urandom"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(9)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/random"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(8)
            .file_mode(0o0666u32)
            .build()
            .unwrap(),
    ]
}

pub fn to_sflag(dev_type: LinuxDeviceType) -> SFlag {
    match dev_type {
        LinuxDeviceType::A => SFlag::S_IFBLK | SFlag::S_IFCHR | SFlag::S_IFIFO,
        LinuxDeviceType::B => SFlag::S_IFBLK,
        LinuxDeviceType::C | LinuxDeviceType::U => SFlag::S_IFCHR,
        LinuxDeviceType::P => SFlag::S_IFIFO,
    }
}

pub fn parse_mount(m: &Mount) -> std::result::Result<MountOptionConfig, MountError> {
    let mut flags = MsFlags::empty();
    let mut data = Vec::new();
    let mut mount_attr: Option<linux::MountAttr> = None;

    if let Some(options) = &m.options() {
        for option in options {
            if let Ok(mount_attr_option) = linux::MountAttrOption::from_str(option.as_str()) {
                let (is_clear, flag) = match mount_attr_option {
                    MountAttrOption::MountArrtRdonly(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNosuid(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNodev(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNoexec(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrAtime(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrRelatime(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNoatime(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrStrictAtime(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNoDiratime(is_clear, flag) => (is_clear, flag),
                    MountAttrOption::MountAttrNosymfollow(is_clear, flag) => (is_clear, flag),
                };

                if mount_attr.is_none() {
                    mount_attr = Some(linux::MountAttr {
                        attr_set: 0,
                        attr_clr: 0,
                        propagation: 0,
                        userns_fd: 0,
                    });
                }

                if let Some(mount_attr) = &mut mount_attr {
                    if is_clear {
                        mount_attr.attr_clr |= flag;
                    } else {
                        mount_attr.attr_set |= flag;
                        if flag & linux::MOUNT_ATTR__ATIME == flag {
                            // https://man7.org/linux/man-pages/man2/mount_setattr.2.html
                            // cannot simply specify the access-time setting in attr_set, but must
                            // also include MOUNT_ATTR__ATIME in the attr_clr field.
                            mount_attr.attr_clr |= linux::MOUNT_ATTR__ATIME;
                        }
                    }
                }
                continue;
            }

            if let Some((is_clear, flag)) = match option.as_str() {
                "defaults" => Some((false, MsFlags::empty())),
                "ro" => Some((false, MsFlags::MS_RDONLY)),
                "rw" => Some((true, MsFlags::MS_RDONLY)),
                "suid" => Some((true, MsFlags::MS_NOSUID)),
                "nosuid" => Some((false, MsFlags::MS_NOSUID)),
                "dev" => Some((true, MsFlags::MS_NODEV)),
                "nodev" => Some((false, MsFlags::MS_NODEV)),
                "exec" => Some((true, MsFlags::MS_NOEXEC)),
                "noexec" => Some((false, MsFlags::MS_NOEXEC)),
                "sync" => Some((false, MsFlags::MS_SYNCHRONOUS)),
                "async" => Some((true, MsFlags::MS_SYNCHRONOUS)),
                "dirsync" => Some((false, MsFlags::MS_DIRSYNC)),
                "remount" => Some((false, MsFlags::MS_REMOUNT)),
                "mand" => Some((false, MsFlags::MS_MANDLOCK)),
                "nomand" => Some((true, MsFlags::MS_MANDLOCK)),
                "atime" => Some((true, MsFlags::MS_NOATIME)),
                "noatime" => Some((false, MsFlags::MS_NOATIME)),
                "diratime" => Some((true, MsFlags::MS_NODIRATIME)),
                "nodiratime" => Some((false, MsFlags::MS_NODIRATIME)),
                "bind" => Some((false, MsFlags::MS_BIND)),
                "rbind" => Some((false, MsFlags::MS_BIND | MsFlags::MS_REC)),
                "unbindable" => Some((false, MsFlags::MS_UNBINDABLE)),
                "runbindable" => Some((false, MsFlags::MS_UNBINDABLE | MsFlags::MS_REC)),
                "private" => Some((true, MsFlags::MS_PRIVATE)),
                "rprivate" => Some((true, MsFlags::MS_PRIVATE | MsFlags::MS_REC)),
                "shared" => Some((true, MsFlags::MS_SHARED)),
                "rshared" => Some((true, MsFlags::MS_SHARED | MsFlags::MS_REC)),
                "slave" => Some((true, MsFlags::MS_SLAVE)),
                "rslave" => Some((true, MsFlags::MS_SLAVE | MsFlags::MS_REC)),
                "relatime" => Some((true, MsFlags::MS_RELATIME)),
                "norelatime" => Some((true, MsFlags::MS_RELATIME)),
                "strictatime" => Some((true, MsFlags::MS_STRICTATIME)),
                "nostrictatime" => Some((true, MsFlags::MS_STRICTATIME)),
                unknown => {
                    if unknown == "idmap" || unknown == "ridmap" {
                        return Err(MountError::UnsupportedMountOption(unknown.to_string()));
                    }
                    None
                }
            } {
                if is_clear {
                    flags &= !flag;
                } else {
                    flags |= flag;
                }
                continue;
            }

            data.push(option.as_str());
        }
    }
    Ok(MountOptionConfig {
        flags,
        data: data.join(","),
        rec_attr: mount_attr,
    })
}

#[cfg(test)]
mod tests {
    use crate::syscall::linux::MountAttr;

    use super::*;

    use anyhow::Result;
    use oci_spec::runtime::MountBuilder;

    #[test]
    fn test_to_sflag() {
        assert_eq!(
            SFlag::S_IFBLK | SFlag::S_IFCHR | SFlag::S_IFIFO,
            to_sflag(LinuxDeviceType::A)
        );
        assert_eq!(SFlag::S_IFBLK, to_sflag(LinuxDeviceType::B));
        assert_eq!(SFlag::S_IFCHR, to_sflag(LinuxDeviceType::C));
        assert_eq!(SFlag::S_IFCHR, to_sflag(LinuxDeviceType::U));
        assert_eq!(SFlag::S_IFIFO, to_sflag(LinuxDeviceType::P));
    }

    #[test]
    fn test_parse_mount() -> Result<()> {
        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/proc"))
                .typ("proc")
                .source(PathBuf::from("proc"))
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::empty(),
                data: "".to_string(),
                rec_attr: None,
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/dev"))
                .typ("tmpfs")
                .source(PathBuf::from("tmpfs"))
                .options(vec![
                    "nosuid".to_string(),
                    "strictatime".to_string(),
                    "mode=755".to_string(),
                    "size=65536k".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID,
                data: "mode=755,size=65536k".to_string(),
                rec_attr: None,
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/dev/pts"))
                .typ("devpts")
                .source(PathBuf::from("devpts"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "newinstance".to_string(),
                    "ptmxmode=0666".to_string(),
                    "mode=0620".to_string(),
                    "gid=5".to_string(),
                ])
                .build()
                .unwrap(),
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                data: "newinstance,ptmxmode=0666,mode=0620,gid=5".to_string(),
                rec_attr: None
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/dev/shm"))
                .typ("tmpfs")
                .source(PathBuf::from("shm"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                    "mode=1777".to_string(),
                    "size=65536k".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                data: "mode=1777,size=65536k".to_string(),
                rec_attr: None
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/dev/mqueue"))
                .typ("mqueue")
                .source(PathBuf::from("mqueue"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                ])
                .build()
                .unwrap(),
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                data: "".to_string(),
                rec_attr: None
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/sys"))
                .typ("sysfs")
                .source(PathBuf::from("sysfs"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                    "ro".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_RDONLY,
                data: "".to_string(),
                rec_attr: None,
            },
            mount_option_config
        );

        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .destination(PathBuf::from("/sys/fs/cgroup"))
                .typ("cgroup")
                .source(PathBuf::from("cgroup"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                    "relatime".to_string(),
                    "ro".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_RDONLY,
                data: "".to_string(),
                rec_attr: None
            },
            mount_option_config,
        );

        // this case is just for coverage purpose
        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .options(vec![
                    "defaults".to_string(),
                    "ro".to_string(),
                    "rw".to_string(),
                    "suid".to_string(),
                    "nosuid".to_string(),
                    "dev".to_string(),
                    "nodev".to_string(),
                    "exec".to_string(),
                    "noexec".to_string(),
                    "sync".to_string(),
                    "async".to_string(),
                    "dirsync".to_string(),
                    "remount".to_string(),
                    "mand".to_string(),
                    "nomand".to_string(),
                    "atime".to_string(),
                    "noatime".to_string(),
                    "diratime".to_string(),
                    "nodiratime".to_string(),
                    "bind".to_string(),
                    "rbind".to_string(),
                    "unbindable".to_string(),
                    "runbindable".to_string(),
                    "private".to_string(),
                    "rprivate".to_string(),
                    "shared".to_string(),
                    "rshared".to_string(),
                    "slave".to_string(),
                    "rslave".to_string(),
                    "relatime".to_string(),
                    "norelatime".to_string(),
                    "strictatime".to_string(),
                    "nostrictatime".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_REMOUNT
                    | MsFlags::MS_DIRSYNC
                    | MsFlags::MS_NOATIME
                    | MsFlags::MS_NODIRATIME
                    | MsFlags::MS_BIND
                    | MsFlags::MS_UNBINDABLE,
                data: "".to_string(),
                rec_attr: None,
            },
            mount_option_config
        );

        // this case is just for coverage purpose
        let mount_option_config = parse_mount(
            &MountBuilder::default()
                .options(vec![
                    "rro".to_string(),
                    "rrw".to_string(),
                    "rnosuid".to_string(),
                    "rsuid".to_string(),
                    "rnodev".to_string(),
                    "rdev".to_string(),
                    "rnoexec".to_string(),
                    "rexec".to_string(),
                    "rnodiratime".to_string(),
                    "rdiratime".to_string(),
                    "rrelatime".to_string(),
                    "rnorelatime".to_string(),
                    "rnoatime".to_string(),
                    "ratime".to_string(),
                    "rstrictatime".to_string(),
                    "rnostrictatime".to_string(),
                    "rnosymfollow".to_string(),
                    "rsymfollow".to_string(),
                ])
                .build()?,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::empty(),
                data: "".to_string(),
                rec_attr: Some(MountAttr::all())
            },
            mount_option_config
        );

        Ok(())
    }
}
