use anyhow::{anyhow, Result};
use nix::{mount::MsFlags, sys::stat::SFlag, NixPath};
use oci_spec::runtime::{LinuxDevice, LinuxDeviceBuilder, LinuxDeviceType, Mount};
use procfs::process::MountInfo;
use std::path::{Path, PathBuf};

pub fn default_devices() -> Vec<LinuxDevice> {
    vec![
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/null"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(3)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/zero"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(5)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/full"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(7)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/tty"))
            .typ(LinuxDeviceType::C)
            .major(5)
            .minor(0)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/urandom"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(9)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/random"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(8)
            .file_mode(0o066u32)
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

pub fn parse_mount(m: &Mount) -> (MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut data = Vec::new();
    if let Some(options) = &m.options() {
        for s in options {
            if let Some((is_clear, flag)) = match s.as_str() {
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
                _ => None,
            } {
                if is_clear {
                    flags &= !flag;
                } else {
                    flags |= flag;
                }
            } else {
                data.push(s.as_str());
            };
        }
    }
    (flags, data.join(","))
}

/// Find parent mount of rootfs in given mount infos
pub fn find_parent_mount<'a>(rootfs: &Path, mount_infos: &'a [MountInfo]) -> Result<&'a MountInfo> {
    // find the longest mount point
    let parent_mount_info = mount_infos
        .iter()
        .filter(|mi| rootfs.starts_with(&mi.mount_point))
        .max_by(|mi1, mi2| mi1.mount_point.len().cmp(&mi2.mount_point.len()))
        .ok_or_else(|| anyhow!("couldn't find parent mount of {}", rootfs.display()))?;
    Ok(parent_mount_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use oci_spec::runtime::MountBuilder;

    #[test]
    fn test_find_parent_mount() -> anyhow::Result<()> {
        let mount_infos = vec![
            MountInfo {
                mnt_id: 11,
                pid: 10,
                majmin: "".to_string(),
                root: "/".to_string(),
                mount_point: PathBuf::from("/"),
                mount_options: Default::default(),
                opt_fields: vec![],
                fs_type: "ext4".to_string(),
                mount_source: Some("/dev/sda1".to_string()),
                super_options: Default::default(),
            },
            MountInfo {
                mnt_id: 12,
                pid: 11,
                majmin: "".to_string(),
                root: "/".to_string(),
                mount_point: PathBuf::from("/proc"),
                mount_options: Default::default(),
                opt_fields: vec![],
                fs_type: "proc".to_string(),
                mount_source: Some("proc".to_string()),
                super_options: Default::default(),
            },
        ];

        let res = find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos)
            .context("Failed to get parent mount")?;
        assert_eq!(res.mnt_id, 11);
        Ok(())
    }

    #[test]
    fn test_find_parent_mount_with_empty_mount_infos() {
        let mount_infos = vec![];
        let res = find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos);
        assert!(res.is_err());
    }

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
    fn test_parse_mount() {
        assert_eq!(
            (MsFlags::empty(), "".to_string()),
            parse_mount(
                &MountBuilder::default()
                    .destination(PathBuf::from("/proc"))
                    .typ("proc")
                    .source(PathBuf::from("proc"))
                    .build()
                    .unwrap()
            )
        );
        assert_eq!(
            (MsFlags::MS_NOSUID, "mode=755,size=65536k".to_string()),
            parse_mount(
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
                    .build()
                    .unwrap()
            )
        );
        assert_eq!(
            (
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                "newinstance,ptmxmode=0666,mode=0620,gid=5".to_string()
            ),
            parse_mount(
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
                    .unwrap()
            )
        );
        assert_eq!(
            (
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                "mode=1777,size=65536k".to_string()
            ),
            parse_mount(
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
                    .build()
                    .unwrap()
            )
        );
        assert_eq!(
            (
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                "".to_string()
            ),
            parse_mount(
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
                    .unwrap()
            )
        );
        assert_eq!(
            (
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
                "".to_string()
            ),
            parse_mount(
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
                    .build()
                    .unwrap()
            )
        );
        assert_eq!(
            (
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
                "".to_string()
            ),
            parse_mount(
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
                    .build()
                    .unwrap()
            )
        );
        // this case is just for coverage purpose
        assert_eq!(
            (
                MsFlags::MS_NOSUID
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_REMOUNT
                    | MsFlags::MS_DIRSYNC
                    | MsFlags::MS_NOATIME
                    | MsFlags::MS_NODIRATIME
                    | MsFlags::MS_BIND
                    | MsFlags::MS_UNBINDABLE,
                "".to_string()
            ),
            parse_mount(
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
                    .build()
                    .unwrap()
            )
        );
    }
}
