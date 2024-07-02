use std::path::PathBuf;
use std::str::FromStr;

use nix::mount::MsFlags;
use nix::sys::stat::SFlag;
use oci_spec::runtime::{LinuxDevice, LinuxDeviceBuilder, LinuxDeviceType, LinuxIdMapping, Mount};
use crate::rootfs::mount::MountError::Custom;

use super::mount::MountError;
use crate::syscall::linux::{self, MountRecursive};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountOptionConfig {
    /// Mount Flags.
    pub flags: MsFlags,

    /// Mount data applied to the mount.
    pub data: String,

    /// RecAttr represents mount properties to be applied recursively.
    pub rec_attr: Option<linux::MountAttr>,

    /// Mapping is the MOUNT_ATTR_IDMAP configuration for the mount. If non-nil,
    /// the mount is configured to use MOUNT_ATTR_IDMAP-style id mappings.
    pub id_mapping: Option<MountIDMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountIDMapping {
    /// Recursive indicates if the mapping needs to be recursive.
    pub recursive: bool,

    /// UserNSPath is a path to a user namespace that indicates the necessary
    /// id-mappings for MOUNT_ATTR_IDMAP. If set to non-"", UIDMappings and
    /// GIDMappings must be set to nil.
    pub user_ns_path: String,

    /// UIDMappings is the uid mapping set for this mount, to be used with
    /// MOUNT_ATTR_IDMAP.
    pub uid_mappings: Option<Vec<IDMap>>,

    /// GIDMappings is the gid mapping set for this mount, to be used with
    /// MOUNT_ATTR_IDMAP.
    pub gid_mappings: Option<Vec<IDMap>>,
}

/// IDMap represents UID/GID Mappings for User Namespaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IDMap {
    pub container_id: u32,
    pub host_id: u32,
    pub size: u32,
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

pub fn parse_mount(m: &Mount,ns_ptah: Option<PathBuf>) -> std::result::Result<MountOptionConfig, MountError> {
    let mut flags = MsFlags::empty();
    let mut data = Vec::new();
    let mut mount_attr: Option<linux::MountAttr> = None;
    let mut id_mapping: MountIDMapping = MountIDMapping {
        recursive: false,
        user_ns_path: "".to_string(),
        uid_mappings: None,
        gid_mappings: None,
    };
    if let Some(options) = &m.options() {
        for option in options {
            if let Ok(mount_attr_option) = linux::MountRecursive::from_str(option.as_str()) {
                // Some options aren't corresponding to the mount flags.
                // These options need `AT_RECURSIVE` options.
                // ref: https://github.com/opencontainers/runtime-spec/blob/main/config.md#linux-mount-options
                let (is_clear, flag) = match mount_attr_option {
                    MountRecursive::Rdonly(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Nosuid(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Nodev(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Noexec(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Atime(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Relatime(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Noatime(is_clear, flag) => (is_clear, flag),
                    MountRecursive::StrictAtime(is_clear, flag) => (is_clear, flag),
                    MountRecursive::NoDiratime(is_clear, flag) => (is_clear, flag),
                    MountRecursive::Nosymfollow(is_clear, flag) => (is_clear, flag),
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
                _unknown => {
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

            id_mapping = match option.as_str() {
                "idmap" => MountIDMapping {
                    recursive: false,
                    user_ns_path: "".to_string(),
                    uid_mappings: None,
                    gid_mappings: None,
                },
                "ridmap" => MountIDMapping {
                    recursive: true,
                    user_ns_path: "".to_string(),
                    uid_mappings: None,
                    gid_mappings: None,
                },
                _ => id_mapping,
            };

            data.push(option.as_str());
        }
    }

    if m.gid_mappings().is_some() || m.uid_mappings().is_some() {
        id_mapping.uid_mappings = to_config_idmap(m.uid_mappings());
        id_mapping.gid_mappings = to_config_idmap(m.gid_mappings());
    }
    if let Some(path) = ns_ptah {
        id_mapping.user_ns_path = path.to_str().unwrap().to_string();
    }
    Ok(MountOptionConfig {
        flags,
        data: data.join(","),
        rec_attr: mount_attr,
        id_mapping: Some(id_mapping),
    })
}


pub fn to_config_idmap(ids: &Option<Vec<LinuxIdMapping>>) -> Option<Vec<IDMap>> {
    if ids.is_none() {
        return None;
    }
    let mut idmaps = Vec::new();
    if let Some(ids_tmp) = ids {
        for id in ids_tmp {
            let idmap = IDMap {
                container_id: id.container_id(),
                host_id: id.host_id(),
                size: id.size(),
            };
            idmaps.push(idmap);
        }
    }
    return Some(idmaps);
}

pub fn check_idmap_mounts(mo_cfg: MountOptionConfig) -> Result<(), MountError> {
    if mo_cfg.id_mapping.is_none() {
        return Ok(());
    }
    if let Some(rec) = mo_cfg.rec_attr {
        if (rec.attr_set | rec.attr_clr) & linux::MOUNT_ATTR_IDMAP != 0 {
            return Err(Custom("mount configuration cannot contain rec_attr for MOUNT_ATTR_IDMAP".to_string()));
        }
    }
    if let Some(m) = mo_cfg.id_mapping {
        if m.user_ns_path == "" {
            if m.gid_mappings.is_none() || m.uid_mappings.is_none() {
                return Err(Custom("id-mapped mounts must have both uid and gid mappings specified".to_string()));
            }
        } else {
            if m.gid_mappings.is_some() || m.uid_mappings.is_some() {
                return Err(Custom("[internal error] id-mapped mounts cannot have both userns_path and uid and gid mappings specified".to_string()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use oci_spec::runtime::MountBuilder;

    use super::*;
    use crate::syscall::linux::MountAttr;

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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::empty(),
                data: "".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID,
                data: "mode=755,size=65536k".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                data: "newinstance,ptmxmode=0666,mode=0620,gid=5".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                data: "mode=1777,size=65536k".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                data: "".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_RDONLY,
                data: "".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_RDONLY,
                data: "".to_string(),
                rec_attr: None,
                id_mapping: None,
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
            None,
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
                id_mapping: None,
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
            None,
        )?;
        assert_eq!(
            MountOptionConfig {
                flags: MsFlags::empty(),
                data: "".to_string(),
                rec_attr: Some(MountAttr::all()),
                id_mapping: None,
            },
            mount_option_config
        );

        Ok(())
    }
}
