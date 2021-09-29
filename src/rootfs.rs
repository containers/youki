//! During kernel initialization, a minimal replica of the ramfs filesystem is loaded, called rootfs.
//! Most systems mount another filesystem over it

use crate::syscall::Syscall;
use crate::utils::PathBufExt;
use anyhow::{anyhow, bail, Context, Result};
use nix::errno::Errno;
use nix::fcntl::{open, OFlag};
use nix::mount::MsFlags;
use nix::sys::stat::{mknod, umask};
use nix::sys::stat::{Mode, SFlag};
use nix::unistd::{chown, close};
use nix::unistd::{Gid, Uid};
use nix::NixPath;
use oci_spec::runtime::{Linux, LinuxDevice, LinuxDeviceBuilder, LinuxDeviceType, Mount, Spec};
use procfs::process::{MountInfo, MountOptFields, Process};
use std::fs::OpenOptions;
use std::fs::{canonicalize, create_dir_all, remove_file};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

pub fn prepare_rootfs(
    spec: &Spec,
    rootfs: &Path,
    bind_devices: bool,
    command: &dyn Syscall,
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

    command
        .mount(None, Path::new("/"), None, flags, None)
        .context("failed to mount rootfs")?;

    make_parent_mount_private(rootfs, &Process::myself()?.mountinfo()?, command)
        .context("failed to change parent mount of rootfs private")?;

    log::debug!("mount root fs {:?}", rootfs);
    command.mount(
        Some(rootfs),
        rootfs,
        None,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None,
    )?;

    if let Some(mounts) = spec.mounts() {
        for mount in mounts {
            log::debug!("Mount... {:?}", mount);
            let (flags, data) = parse_mount(mount);
            let mount_label = linux.mount_label().as_ref();
            if *mount.typ() == Some("cgroup".to_string()) {
                // skip
                log::warn!("A feature of cgroup is unimplemented.");
            } else if *mount.destination() == PathBuf::from("/dev") {
                mount_to_container(
                    mount,
                    rootfs,
                    flags & !MsFlags::MS_RDONLY,
                    &data,
                    mount_label,
                    command,
                )
                .with_context(|| format!("Failed to mount /dev: {:?}", mount))?;
            } else {
                mount_to_container(mount, rootfs, flags, &data, mount_label, command)
                    .with_context(|| format!("Failed to mount: {:?}", mount))?;
            }
        }
    }

    setup_default_symlinks(rootfs).context("Failed to setup default symlinks")?;
    if let Some(added_devices) = linux.devices() {
        create_devices(
            rootfs,
            default_devices().iter().chain(added_devices),
            bind_devices,
            command,
        )
    } else {
        create_devices(rootfs, &default_devices(), bind_devices, command)
    }?;

    setup_ptmx(rootfs)?;
    Ok(())
}

fn setup_ptmx(rootfs: &Path) -> Result<()> {
    if let Err(e) = remove_file(rootfs.join("dev/ptmx")) {
        if e.kind() != ::std::io::ErrorKind::NotFound {
            bail!("could not delete /dev/ptmx")
        }
    }

    symlink("pts/ptmx", rootfs.join("dev/ptmx")).context("failed to symlink ptmx")?;
    Ok(())
}

fn setup_default_symlinks(rootfs: &Path) -> Result<()> {
    if Path::new("/proc/kcore").exists() {
        symlink("/proc/kcore", rootfs.join("dev/kcore")).context("Failed to symlink kcore")?;
    }

    let defaults = [
        ("/proc/self/fd", "dev/fd"),
        ("/proc/self/fd/0", "dev/stdin"),
        ("/proc/self/fd/1", "dev/stdout"),
        ("/proc/self/fd/2", "dev/stderr"),
    ];
    for (src, dst) in defaults {
        symlink(src, rootfs.join(dst)).context("Fail to symlink defaults")?;
    }

    Ok(())
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

fn create_devices<'a, I>(rootfs: &Path, devices: I, bind: bool, command: &dyn Syscall) -> Result<()>
where
    I: IntoIterator<Item = &'a LinuxDevice>,
{
    let old_mode = umask(Mode::from_bits_truncate(0o000));
    if bind {
        let _ = devices
            .into_iter()
            .map(|dev| {
                if !dev.path().starts_with("/dev") {
                    panic!("{} is not a valid device path", dev.path().display());
                }

                bind_dev(rootfs, dev, command)
            })
            .collect::<Result<Vec<_>>>()?;
    } else {
        devices
            .into_iter()
            .map(|dev| {
                if !dev.path().starts_with("/dev") {
                    panic!("{} is not a valid device path", dev.path().display());
                }

                mknod_dev(rootfs, dev)
            })
            .collect::<Result<Vec<_>>>()?;
    }
    umask(old_mode);

    Ok(())
}

fn bind_dev(rootfs: &Path, dev: &LinuxDevice, command: &dyn Syscall) -> Result<()> {
    let full_container_path = rootfs.join(dev.path().as_in_container()?);

    let fd = open(
        &full_container_path,
        OFlag::O_RDWR | OFlag::O_CREAT,
        Mode::from_bits_truncate(0o644),
    )?;
    close(fd)?;
    command.mount(
        Some(dev.path()),
        &full_container_path,
        Some("bind"),
        MsFlags::MS_BIND,
        None,
    )?;

    Ok(())
}

fn to_sflag(dev_type: LinuxDeviceType) -> SFlag {
    match dev_type {
        LinuxDeviceType::A => SFlag::S_IFBLK | SFlag::S_IFCHR | SFlag::S_IFIFO,
        LinuxDeviceType::B => SFlag::S_IFBLK,
        LinuxDeviceType::C | LinuxDeviceType::U => SFlag::S_IFCHR,
        LinuxDeviceType::P => SFlag::S_IFIFO,
    }
}

fn mknod_dev(rootfs: &Path, dev: &LinuxDevice) -> Result<()> {
    fn makedev(major: i64, minor: i64) -> u64 {
        ((minor & 0xff)
            | ((major & 0xfff) << 8)
            | ((minor & !0xff) << 12)
            | ((major & !0xfff) << 32)) as u64
    }

    let full_container_path = rootfs.join(dev.path().as_in_container()?);
    mknod(
        &full_container_path,
        to_sflag(dev.typ()),
        Mode::from_bits_truncate(dev.file_mode().unwrap_or(0)),
        makedev(dev.major(), dev.minor()),
    )?;
    chown(
        &full_container_path,
        dev.uid().map(Uid::from_raw),
        dev.gid().map(Gid::from_raw),
    )?;

    Ok(())
}

fn mount_to_container(
    m: &Mount,
    rootfs: &Path,
    flags: MsFlags,
    data: &str,
    label: Option<&String>,
    command: &dyn Syscall,
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
        create_dir_all(&dest).with_context(|| format!("Failed to create device: {:?}", dest))?;
        PathBuf::from(source)
    };

    if let Err(errno) = command.mount(Some(&*src), dest, typ, flags, Some(&*d)) {
        if !matches!(errno, Errno::EINVAL) {
            bail!("mount of {:?} failed", m.destination());
        }
        command.mount(Some(&*src), dest, typ, flags, Some(data))?;
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
        command.mount(Some(dest), dest, None, flags | MsFlags::MS_REMOUNT, None)?;
    }
    Ok(())
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

/// Make parent mount of rootfs private if it was shared, which is required by pivot_root.
/// It also makes sure following bind mount does not propagate in other namespaces.
fn make_parent_mount_private(
    rootfs: &Path,
    mount_infos: &[MountInfo],
    command: &dyn Syscall,
) -> Result<()> {
    let parent_mount = find_parent_mount(rootfs, mount_infos)?;

    // check parent mount has 'shared' propagation type
    if parent_mount
        .opt_fields
        .iter()
        .any(|field| matches!(field, MountOptFields::Shared(_)))
    {
        command.mount(
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
pub fn adjust_root_mount_propagation(linux: &Linux, command: &dyn Syscall) -> Result<()> {
    let rootfs_propagation = linux.rootfs_propagation().as_deref();
    let flags = match rootfs_propagation {
        Some("shared") => Some(MsFlags::MS_SHARED),
        Some("unbindable") => Some(MsFlags::MS_UNBINDABLE),
        _ => None,
    };

    if let Some(flags) = flags {
        log::debug!("make root mount {:?}", flags);
        command.mount(None, Path::new("/"), None, flags, None)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::syscall::test::{MountArgs, TestHelperSyscall};
    use anyhow::{Context, Result};
    use nix::mount::MsFlags;
    use oci_spec::runtime::LinuxBuilder;
    use procfs::process::{MountInfo, MountOptFields};
    use std::path::{Path, PathBuf};

    // sample root mount info
    fn root_mount() -> MountInfo {
        MountInfo {
            mnt_id: 11,
            pid: 10,
            majmin: String::from(""),
            root: String::from("/"),
            mount_point: PathBuf::from("/"),
            mount_options: Default::default(),
            opt_fields: vec![],
            fs_type: String::from("ext4"),
            mount_source: Some(String::from("/dev/sda1")),
            super_options: Default::default(),
        }
    }

    // sample proc mount info
    fn proc_mount() -> MountInfo {
        MountInfo {
            mnt_id: 12,
            pid: 11,
            majmin: String::from(""),
            root: String::from("/"),
            mount_point: PathBuf::from("/proc"),
            mount_options: Default::default(),
            opt_fields: vec![],
            fs_type: String::from("proc"),
            mount_source: Some(String::from("proc")),
            super_options: Default::default(),
        }
    }

    #[test]
    fn test_find_parent_mount() -> Result<()> {
        let mount_infos = vec![root_mount(), proc_mount()];

        let res = super::find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos)
            .context("Failed to get parent mount")?;
        assert_eq!(res.mnt_id, 11);
        Ok(())
    }

    #[test]
    fn test_find_parent_mount_with_empty_mount_infos() {
        let mount_infos = vec![];
        let res = super::find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos);
        assert!(res.is_err());
    }

    #[test]
    fn test_make_parent_mount_private() -> Result<()> {
        let opt_fields_list = [
            vec![MountOptFields::Shared(1)],
            vec![MountOptFields::Master(1)],
            vec![],
        ];

        let expected_mount_args_list = [
            vec![MountArgs {
                source: None,
                target: PathBuf::from("/"),
                fstype: None,
                flags: MsFlags::MS_PRIVATE,
                data: None,
            }],
            vec![],
            vec![],
        ];

        for (opt_fields, expected_mount_args) in
            opt_fields_list.iter().zip(expected_mount_args_list.iter())
        {
            let mut root_mount = root_mount();
            root_mount.opt_fields = opt_fields.to_owned();
            let mount_infos = vec![root_mount, proc_mount()];
            let command = TestHelperSyscall::default();

            super::make_parent_mount_private(Path::new("/path/to/rootfs"), &mount_infos, &command)?;

            assert_eq!(
                command.get_mount_args(),
                *expected_mount_args,
                "failed with opt_fields of mount info: {:?}",
                opt_fields
            );
        }

        Ok(())
    }

    #[test]
    fn test_adjust_root_mount_propagation() -> Result<()> {
        let propagation_type_list = vec!["shared", "private", "slave", "unbindable"];

        let expected_mount_args_list = vec![
            vec![MountArgs {
                source: None,
                target: PathBuf::from("/"),
                fstype: None,
                flags: MsFlags::MS_SHARED,
                data: None,
            }],
            vec![],
            vec![],
            vec![MountArgs {
                source: None,
                target: PathBuf::from("/"),
                fstype: None,
                flags: MsFlags::MS_UNBINDABLE,
                data: None,
            }],
        ];

        for (propagation_type, expected_mount_args) in propagation_type_list
            .into_iter()
            .zip(expected_mount_args_list.into_iter())
        {
            let linux = LinuxBuilder::default()
                .rootfs_propagation(propagation_type)
                .build()
                .expect("could not build");
            let command = TestHelperSyscall::default();

            super::adjust_root_mount_propagation(&linux, &command)?;

            assert_eq!(
                command.get_mount_args(),
                expected_mount_args,
                "failed with propagation type: {}",
                propagation_type
            );
        }

        Ok(())
    }
}
