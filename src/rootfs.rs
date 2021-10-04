//! During kernel initialization, a minimal replica of the ramfs filesystem is loaded, called rootfs.
//! Most systems mount another filesystem over it

use crate::syscall::{syscall::create_syscall, Syscall};
use crate::utils::PathBufExt;
use anyhow::{anyhow, bail, Context, Result};
use cgroups::common::CgroupSetup::{Hybrid, Legacy, Unified};
use nix::errno::Errno;
use nix::fcntl::{open, OFlag};
use nix::mount::MsFlags;
use nix::sys::stat::umask;
use nix::sys::stat::{Mode, SFlag};
use nix::unistd::close;
use nix::unistd::{Gid, Uid};
use nix::NixPath;
use oci_spec::runtime::{
    Linux, LinuxDevice, LinuxDeviceBuilder, LinuxDeviceType, Mount, MountBuilder, Spec,
};
use procfs::process::{MountInfo, MountOptFields, Process};
use std::fs::OpenOptions;
use std::fs::{canonicalize, create_dir_all, remove_file};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct MountOptions<'a> {
    root: &'a Path,
    label: Option<&'a str>,
    cgroup_ns: bool,
}

/// Holds information about rootfs
pub struct RootFS {
    syscall: Box<dyn Syscall>,
}

impl Default for RootFS {
    fn default() -> Self {
        Self::new()
    }
}

impl RootFS {
    pub fn new() -> RootFS {
        RootFS {
            syscall: create_syscall(),
        }
    }

    pub fn prepare_rootfs(
        &self,
        spec: &Spec,
        rootfs: &Path,
        bind_devices: bool,
        cgroup_ns: bool,
    ) -> Result<()> {
        log::debug!("Prepare rootfs: {:?}", rootfs);
        let mut flags = MsFlags::MS_REC;
        let linux = spec.linux().as_ref().context("no linux in spec")?;

        match linux.rootfs_propagation().as_deref() {
            Some("shared") => flags |= MsFlags::MS_SHARED,
            Some("private") => flags |= MsFlags::MS_PRIVATE,
            Some("slave" | "unbindable") | None => flags |= MsFlags::MS_SLAVE,
            Some(uknown) => bail!("unknown rootfs_propagation: {}", uknown),
        }

        self.syscall
            .mount(None, Path::new("/"), None, flags, None)
            .context("failed to mount rootfs")?;

        self.make_parent_mount_private(rootfs)
            .context("failed to change parent mount of rootfs private")?;

        log::debug!("mount root fs {:?}", rootfs);
        self.syscall.mount(
            Some(rootfs),
            rootfs,
            None,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None,
        )?;

        let global_options = MountOptions {
            root: rootfs,
            label: linux.mount_label().as_deref(),
            cgroup_ns,
        };

        if let Some(mounts) = spec.mounts() {
            for mount in mounts {
                self.setup_mount(mount, &global_options)
                    .with_context(|| format!("failed to setup mount {:#?}", mount))?;
            }
        }

        self.setup_kcore_symlink(rootfs)
            .context("failed to  setup kcore symlink")?;
        self.setup_default_symlinks(rootfs)
            .context("failed to setup default symlinks")?;

        if let Some(added_devices) = linux.devices() {
            self.create_devices(
                rootfs,
                default_devices().iter().chain(added_devices),
                bind_devices,
            )
        } else {
            self.create_devices(rootfs, &default_devices(), bind_devices)
        }?;

        self.setup_ptmx(rootfs)?;
        Ok(())
    }

    fn setup_mount(&self, mount: &Mount, options: &MountOptions) -> Result<()> {
        log::debug!("Mounting {:?}", mount);
        let (flags, data) = parse_mount(mount);

        match mount.typ().as_deref() {
            Some("cgroup") => {
                match cgroups::common::get_cgroup_setup()
                    .context("failed to determine cgroup setup")?
                {
                    Legacy | Hybrid => self
                        .mount_cgroup_v1(mount, options)
                        .context("failed to mount cgroup v1")?,
                    Unified => self
                        .mount_cgroup_v2(mount, options, flags, &data)
                        .context("failed to mount cgroup v2")?,
                }
            }
            _ => {
                if *mount.destination() == PathBuf::from("/dev") {
                    self.mount_to_container(
                        mount,
                        options.root,
                        flags & !MsFlags::MS_RDONLY,
                        &data,
                        options.label,
                    )
                    .with_context(|| format!("failed to mount /dev: {:?}", mount))?;
                } else {
                    self.mount_to_container(mount, options.root, flags, &data, options.label)
                        .with_context(|| format!("failed to mount: {:?}", mount))?;
                }
            }
        }

        Ok(())
    }

    fn mount_cgroup_v1(&self, mount: &Mount, options: &MountOptions) -> Result<()> {
        // create tmpfs into which the cgroup subsystems will be mounted
        let tmpfs = MountBuilder::default()
            .source("tmpfs")
            .typ("tmpfs")
            .destination(mount.destination())
            .options(
                ["noexec", "nosuid", "nodev", "mode=755"]
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<String>>(),
            )
            .build()
            .context("failed to build tmpfs for cgroup")?;

        self.setup_mount(&tmpfs, options)
            .context("failed to mount tmpfs for cgroup")?;

        // get all cgroup mounts on the host system
        let mount_points: Vec<PathBuf> = cgroups::v1::util::list_subsystem_mount_points()
            .context("failed to get subsystem mount points")?
            .into_iter()
            .filter(|p| p.as_path().starts_with("/sys/fs"))
            .collect();
        log::debug!("{:?}", mount_points);

        // setup cgroup mounts for container
        let cgroup_root = options
            .root
            .join_safely(mount.destination())
            .context("could not join rootfs with cgroup destination")?;
        for mount_point in mount_points {
            if let Some(subsystem_name) = mount_point.file_name().and_then(|n| n.to_str()) {
                let cgroup_mount = MountBuilder::default()
                    .source("cgroup")
                    .typ("cgroup")
                    .destination(mount.destination().join(subsystem_name))
                    .options(
                        ["noexec", "nosuid", "nodev"]
                            .iter()
                            .map(|o| o.to_string())
                            .collect::<Vec<String>>(),
                    )
                    .build()
                    .with_context(|| format!("failed to build {}", subsystem_name))?;

                if subsystem_name == "systemd" {
                    continue;
                }

                if options.cgroup_ns {
                    self.setup_namespaced_hierarchy(&cgroup_mount, options, subsystem_name)?;
                    self.setup_comount_symlinks(&cgroup_root, subsystem_name)?;
                } else {
                    log::warn!("cgroup mounts are currently only suported with cgroup namespaces")
                }
            } else {
                log::warn!("could not get subsystem name from {:?}", mount_point);
            }
        }

        Ok(())
    }

    // On some distros cgroup subsystems are comounted e.g. cpu,cpuacct or net_cls,net_prio. These systems
    // have to be comounted in the container as well as the kernel will reject trying to mount them separately.
    fn setup_namespaced_hierarchy(
        &self,
        cgroup_mount: &Mount,
        options: &MountOptions,
        subsystem_name: &str,
    ) -> Result<()> {
        log::debug!("Mounting cgroup subsystem: {:?}", subsystem_name);
        self.mount_to_container(
            cgroup_mount,
            options.root,
            MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            subsystem_name,
            options.label,
        )
        .with_context(|| format!("failed to mount {:?}", cgroup_mount))
    }

    // Create symlinks for subsystems that have been comounted e.g. cpu -> cpu,cpuacct, cpuacct -> cpu,cpuacct
    fn setup_comount_symlinks(&self, cgroup_root: &Path, subsystem_name: &str) -> Result<()> {
        if !subsystem_name.contains(',') {
            return Ok(());
        }

        for comount in subsystem_name.split_terminator(',') {
            let link = cgroup_root.join(comount);
            self.syscall
                .symlink(Path::new(subsystem_name), &link)
                .with_context(|| format!("failed to symlink {:?} to {:?}", link, subsystem_name))?;
        }

        Ok(())
    }

    fn mount_cgroup_v2(&self, _: &Mount, _: &MountOptions, _: MsFlags, _: &str) -> Result<()> {
        log::warn!("Mounting cgroup v2 is not yet supported");
        Ok(())
    }

    fn setup_ptmx(&self, rootfs: &Path) -> Result<()> {
        let ptmx = rootfs.join("dev/ptmx");
        if let Err(e) = remove_file(&ptmx) {
            if e.kind() != ::std::io::ErrorKind::NotFound {
                bail!("could not delete /dev/ptmx")
            }
        }

        self.syscall
            .symlink(Path::new("pts/ptmx"), &ptmx)
            .context("failed to symlink ptmx")?;
        Ok(())
    }

    // separating kcore symlink out from setup_default_symlinks for a better way to do the unit test,
    // since not every architecture has /proc/kcore file.
    fn setup_kcore_symlink(&self, rootfs: &Path) -> Result<()> {
        if Path::new("/proc/kcore").exists() {
            self.syscall
                .symlink(Path::new("/proc/kcore"), &rootfs.join("dev/kcore"))
                .context("Failed to symlink kcore")?;
        }
        Ok(())
    }

    fn setup_default_symlinks(&self, rootfs: &Path) -> Result<()> {
        let defaults = [
            ("/proc/self/fd", "dev/fd"),
            ("/proc/self/fd/0", "dev/stdin"),
            ("/proc/self/fd/1", "dev/stdout"),
            ("/proc/self/fd/2", "dev/stderr"),
        ];
        for (src, dst) in defaults {
            self.syscall
                .symlink(Path::new(src), &rootfs.join(dst))
                .context("failed to symlink defaults")?;
        }

        Ok(())
    }

    fn create_devices<'a, I>(&self, rootfs: &Path, devices: I, bind: bool) -> Result<()>
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

                crate::utils::create_dir_all_with_mode(
                    rootfs
                        .join(dev.path().as_in_container()?)
                        .parent()
                        .unwrap_or_else(|| Path::new("/")),
                    dev.uid().unwrap_or(0),
                    Mode::from_bits_truncate(0o755),
                )?;

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
        let full_container_path = rootfs.join(dev.path().as_in_container()?);

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

        let full_container_path = rootfs.join(dev.path().as_in_container()?);
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

    fn mount_to_container(
        &self,
        m: &Mount,
        rootfs: &Path,
        flags: MsFlags,
        data: &str,
        label: Option<&str>,
    ) -> Result<()> {
        let typ = m.typ().as_deref();
        let d = if let Some(l) = label {
            if typ != Some("proc") && typ != Some("sysfs") {
                if data.is_empty() {
                    format!("context=\"{}\"", l)
                } else {
                    format!("{},context=\"{}\"", data, l)
                }
            } else {
                data.to_string()
            }
        } else {
            data.to_string()
        };
        let dest_for_host = format!(
            "{}{}",
            rootfs.to_string_lossy().into_owned(),
            m.destination().display()
        );
        let dest = Path::new(&dest_for_host);
        let source = m.source().as_ref().context("no source in mount spec")?;
        let src = if typ == Some("bind") {
            let src = canonicalize(source)?;
            let dir = if src.is_file() {
                Path::new(&dest).parent().unwrap()
            } else {
                Path::new(&dest)
            };
            create_dir_all(&dir)
                .with_context(|| format!("Failed to create dir for bind mount: {:?}", dir))?;
            if src.is_file() {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&dest)
                    .unwrap();
            }

            src
        } else {
            create_dir_all(&dest)
                .with_context(|| format!("Failed to create device: {:?}", dest))?;
            PathBuf::from(source)
        };

        if let Err(err) = self.syscall.mount(Some(&*src), dest, typ, flags, Some(&*d)) {
            if let Some(errno) = err.downcast_ref() {
                if !matches!(errno, Errno::EINVAL) {
                    bail!("mount of {:?} failed. {}", m.destination(), errno);
                }
            }

            self.syscall
                .mount(Some(&*src), dest, typ, flags, Some(data))?;
        }

        if flags.contains(MsFlags::MS_BIND)
            && flags.intersects(
                !(MsFlags::MS_REC
                    | MsFlags::MS_REMOUNT
                    | MsFlags::MS_BIND
                    | MsFlags::MS_PRIVATE
                    | MsFlags::MS_SHARED
                    | MsFlags::MS_SLAVE),
            )
        {
            self.syscall
                .mount(Some(dest), dest, None, flags | MsFlags::MS_REMOUNT, None)?;
        }
        Ok(())
    }

    /// Make parent mount of rootfs private if it was shared, which is required by pivot_root.
    /// It also makes sure following bind mount does not propagate in other namespaces.
    fn make_parent_mount_private(&self, rootfs: &Path) -> Result<()> {
        let mount_infos = Process::myself()?.mountinfo()?;
        let parent_mount = find_parent_mount(rootfs, &mount_infos)?;

        // check parent mount has 'shared' propagation type
        if parent_mount
            .opt_fields
            .iter()
            .any(|field| matches!(field, MountOptFields::Shared(_)))
        {
            self.syscall.mount(
                None,
                &parent_mount.mount_point,
                None,
                MsFlags::MS_PRIVATE,
                None,
            )?;
        }

        Ok(())
    }

    /// Change propagation type of rootfs as specified in spec.
    pub fn adjust_root_mount_propagation(&self, linux: &Linux) -> Result<()> {
        let rootfs_propagation = linux.rootfs_propagation().as_deref();
        let flags = match rootfs_propagation {
            Some("shared") => Some(MsFlags::MS_SHARED),
            Some("unbindable") => Some(MsFlags::MS_UNBINDABLE),
            _ => None,
        };

        if let Some(flags) = flags {
            log::debug!("make root mount {:?}", flags);
            self.syscall
                .mount(None, Path::new("/"), None, flags, None)?;
        }

        Ok(())
    }
}

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

fn to_sflag(dev_type: LinuxDeviceType) -> SFlag {
    match dev_type {
        LinuxDeviceType::A => SFlag::S_IFBLK | SFlag::S_IFCHR | SFlag::S_IFIFO,
        LinuxDeviceType::B => SFlag::S_IFBLK,
        LinuxDeviceType::C | LinuxDeviceType::U => SFlag::S_IFCHR,
        LinuxDeviceType::P => SFlag::S_IFIFO,
    }
}

fn parse_mount(m: &Mount) -> (MsFlags, String) {
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
fn find_parent_mount<'a>(rootfs: &Path, mount_infos: &'a [MountInfo]) -> Result<&'a MountInfo> {
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
    use crate::syscall::test::{ChownArgs, MknodArgs, MountArgs, TestHelperSyscall};
    use procfs::process::MountInfo;
    use serial_test::serial;

    #[test]
    fn test_find_parent_mount() -> Result<()> {
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

    #[test]
    #[serial]
    fn test_setup_ptmx() {
        {
            let rootfs = RootFS::new();
            assert!(rootfs.setup_ptmx(Path::new("/tmp")).is_ok());
            let want = (PathBuf::from("pts/ptmx"), PathBuf::from("/tmp/dev/ptmx"));
            let got = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_symlink_args()[0];
            assert_eq!(want, *got)
        }
        // make remove_file goes into the bail! path
        {
            open(
                "/tmp/dev",
                OFlag::O_RDWR | OFlag::O_CREAT,
                Mode::from_bits_truncate(0o644),
            )
            .unwrap();

            let rootfs = RootFS::new();
            assert!(rootfs.setup_ptmx(Path::new("/tmp")).is_err());
            assert_eq!(
                0,
                rootfs
                    .syscall
                    .as_any()
                    .downcast_ref::<TestHelperSyscall>()
                    .unwrap()
                    .get_symlink_args()
                    .len()
            );
            remove_file("/tmp/dev").unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_setup_default_symlinks() {
        let rootfs = RootFS::new();
        assert!(rootfs.setup_default_symlinks(Path::new("/tmp")).is_ok());
        let want = vec![
            (PathBuf::from("/proc/self/fd"), PathBuf::from("/tmp/dev/fd")),
            (
                PathBuf::from("/proc/self/fd/0"),
                PathBuf::from("/tmp/dev/stdin"),
            ),
            (
                PathBuf::from("/proc/self/fd/1"),
                PathBuf::from("/tmp/dev/stdout"),
            ),
            (
                PathBuf::from("/proc/self/fd/2"),
                PathBuf::from("/tmp/dev/stderr"),
            ),
        ];
        let got = rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_symlink_args();
        assert_eq!(want, got)
    }

    #[test]
    #[serial]
    fn test_bind_dev() {
        let rootfs = RootFS::new();
        assert!(rootfs
            .bind_dev(
                Path::new("/tmp"),
                &LinuxDeviceBuilder::default()
                    .path(PathBuf::from("/null"))
                    .build()
                    .unwrap(),
            )
            .is_ok());

        let want = MountArgs {
            source: Some(PathBuf::from("/null")),
            target: PathBuf::from("/tmp/null"),
            fstype: Some("bind".to_string()),
            flags: MsFlags::MS_BIND,
            data: None,
        };
        let got = &rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args()[0];
        assert_eq!(want, *got);

        remove_file("/tmp/null").unwrap();
    }

    #[test]
    #[serial]
    fn test_mknod_dev() {
        let rootfs = RootFS::new();
        assert!(rootfs
            .mknod_dev(
                Path::new("/tmp"),
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
            path: PathBuf::from("/tmp/null"),
            kind: SFlag::S_IFCHR,
            perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
            dev: 259,
        };
        let got_mknod = &rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mknod_args()[0];
        assert_eq!(want_mknod, *got_mknod);

        let want_chown = ChownArgs {
            path: PathBuf::from("/tmp/null"),
            owner: Some(Uid::from_raw(1000)),
            group: Some(Gid::from_raw(1000)),
        };
        let got_chown = &rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_chown_args()[0];
        assert_eq!(want_chown, *got_chown);
    }

    #[test]
    #[serial]
    fn test_create_devices() {
        let rootfs = RootFS::new();
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

        assert!(rootfs
            .create_devices(Path::new("/tmp"), &devices, true)
            .is_ok());

        let want = MountArgs {
            source: Some(PathBuf::from("/dev/null")),
            target: PathBuf::from("/tmp/dev/null"),
            fstype: Some("bind".to_string()),
            flags: MsFlags::MS_BIND,
            data: None,
        };
        let got = &rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args()[0];
        assert_eq!(want, *got);

        assert!(rootfs
            .create_devices(Path::new("/tmp"), &devices, false)
            .is_ok());

        let want = MknodArgs {
            path: PathBuf::from("/tmp/dev/null"),
            kind: SFlag::S_IFCHR,
            perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
            dev: 259,
        };
        let got = &rootfs
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mknod_args()[0];
        assert_eq!(want, *got);

        std::fs::remove_dir_all("/tmp/dev").unwrap();
    }
}
