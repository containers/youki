use std::fs::OpenOptions;
use std::fs::{canonicalize, create_dir_all, remove_file};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Result};
use futures::future::{self, try_join_all};
use futures::stream::{self, StreamExt};
use futures::task::SpawnExt;
use nix::errno::Errno;
use nix::fcntl::{open, OFlag};
use nix::mount::mount as nix_mount;
use nix::mount::{umount2, MntFlags, MsFlags};
use nix::sys::stat::Mode;
use nix::sys::stat::{mknod, umask};
use nix::unistd::{chdir, chown, close, fchdir, getcwd, pivot_root};
use nix::unistd::{Gid, Uid};
use nix::NixPath;

use crate::spec::{LinuxDevice, LinuxDeviceType, Mount, Spec};
use crate::utils::PathBufExt;

pub async fn prepare_rootfs(
    spec: Arc<Spec>,
    rootfs: Arc<PathBuf>,
    bind_devices: bool,
) -> Result<()> {
    let mut flags = MsFlags::MS_REC;
    match spec.linux {
        Some(ref linux) => match linux.rootfs_propagation.as_ref() {
            "shared" => flags |= MsFlags::MS_SHARED,
            "private" => flags |= MsFlags::MS_PRIVATE,
            "slave" | "" => flags |= MsFlags::MS_SLAVE,
            _ => panic!(),
        },
        None => flags |= MsFlags::MS_SLAVE,
    };
    nix_mount(None::<&str>, "/", None::<&str>, flags, None::<&str>)?;

    log::debug!("mount root fs {:?}", rootfs);
    nix_mount(
        Some(rootfs.as_ref()),
        rootfs.as_ref(),
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )?;

    let pool = futures::executor::ThreadPool::new()?;
    let can_parall = spec.mounts.clone().into_iter().filter(|m| m.typ != "tmpfs");
    let cannot_parall = spec.mounts.iter().filter(|m| m.typ == "tmpfs");

    for m in cannot_parall {
        let (flags, data) = parse_mount(&m);
        let ml = &spec.linux.as_ref().unwrap().mount_label;
        if m.typ == "cgroup" {
            // skip
            log::warn!("A feature of cgoup is unimplemented.");
        } else if m.destination == PathBuf::from("/dev") {
            mount_to_container(&m, rootfs.as_ref(), flags & !MsFlags::MS_RDONLY, &data, &ml)?;
        } else {
            mount_to_container(&m, rootfs.as_ref(), flags, &data, &ml)?;
        }
    }

    try_join_all(
        stream::iter(can_parall)
            .map(|m| {
                let spec = Arc::clone(&spec);
                let rootfs = Arc::clone(&rootfs);
                pool.spawn_with_handle(async move {
                    let (flags, data) = parse_mount(&m);
                    let ml = &spec.linux.as_ref().unwrap().mount_label;
                    if m.typ == "cgroup" {
                        // skip
                        log::warn!("A feature of cgoup is unimplemented.");
                        Ok(())
                    } else if m.destination == PathBuf::from("/dev") {
                        mount_to_container(
                            &m,
                            rootfs.as_ref(),
                            flags & !MsFlags::MS_RDONLY,
                            &data,
                            &ml,
                        )
                    } else {
                        mount_to_container(&m, rootfs.as_ref(), flags, &data, &ml)
                    }
                })
                .unwrap()
            })
            .collect::<Vec<_>>()
            .await,
    )
    .await?;

    let olddir = getcwd()?;
    chdir(rootfs.as_ref())?;

    setup_default_symlinks(&rootfs.as_ref())?;
    create_devices(&spec.linux.as_ref().unwrap().devices, bind_devices).await?;
    setup_ptmx(rootfs.as_ref())?;

    chdir(&olddir)?;

    Ok(())
}

fn setup_ptmx(rootfs: &Path) -> Result<()> {
    if let Err(e) = remove_file(rootfs.join("dev/ptmx")) {
        if e.kind() != ::std::io::ErrorKind::NotFound {
            bail!("could not delete /dev/ptmx")
        }
    }
    symlink("pts/ptmx", rootfs.join("dev/ptmx"))?;
    Ok(())
}

fn setup_default_symlinks(rootfs: &Path) -> Result<()> {
    if Path::new("/proc/kcore").exists() {
        symlink("/proc/kcore", "dev/kcore")?;
    }

    let defaults = [
        ("/proc/self/fd", "dev/fd"),
        ("/proc/self/fd/0", "dev/stdin"),
        ("/proc/self/fd/1", "dev/stdout"),
        ("/proc/self/fd/2", "dev/stderr"),
    ];
    for &(src, dst) in defaults.iter() {
        symlink(src, rootfs.join(dst))?;
    }
    Ok(())
}

fn default_devices() -> [LinuxDevice; 6] {
    [
        LinuxDevice {
            path: PathBuf::from("/dev/null"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 3,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/zero"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 5,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/full"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 7,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/tty"),
            typ: LinuxDeviceType::C,
            major: 5,
            minor: 0,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/urandom"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 9,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/random"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 8,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
    ]
}

async fn create_devices(devices: &[LinuxDevice], bind: bool) -> Result<()> {
    let old_mode = umask(Mode::from_bits_truncate(0o000));
    if bind {
        future::try_join_all(default_devices().iter().chain(devices).map(|dev| {
            if !dev.path.starts_with("/dev") {
                panic!("{} is not a valid device path", dev.path.display());
            }
            bind_dev(dev)
        }))
        .await?;
    } else {
        future::try_join_all(default_devices().iter().chain(devices).map(|dev| {
            if !dev.path.starts_with("/dev") {
                panic!("{} is not a valid device path", dev.path.display());
            }
            mknod_dev(dev)
        }))
        .await?;
    }
    umask(old_mode);
    Ok(())
}

async fn bind_dev(dev: &LinuxDevice) -> Result<()> {
    let fd = open(
        &dev.path.as_in_container()?,
        OFlag::O_RDWR | OFlag::O_CREAT,
        Mode::from_bits_truncate(0o644),
    )?;
    close(fd)?;
    nix_mount(
        Some(&*dev.path.as_in_container()?),
        &dev.path,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )?;
    Ok(())
}

async fn mknod_dev(dev: &LinuxDevice) -> Result<()> {
    fn makedev(major: u64, minor: u64) -> u64 {
        (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & !0xff) << 12) | ((major & !0xfff) << 32)
    }

    mknod(
        &dev.path.as_in_container()?,
        dev.typ.to_sflag()?,
        Mode::from_bits_truncate(dev.file_mode.unwrap_or(0)),
        makedev(dev.major, dev.minor),
    )?;
    chown(
        &dev.path.as_in_container()?,
        dev.uid.map(Uid::from_raw),
        dev.gid.map(Gid::from_raw),
    )?;
    Ok(())
}

fn mount_to_container(
    m: &Mount,
    rootfs: &Path,
    flags: MsFlags,
    data: &str,
    label: &str,
) -> Result<()> {
    let d = if !label.is_empty() && m.typ != "proc" && m.typ != "sysfs" {
        if data.is_empty() {
            format!("context=\"{}\"", label)
        } else {
            format!("{},context=\"{}\"", data, label)
        }
    } else {
        data.to_string()
    };

    let dest_for_host = format!(
        "{}{}",
        rootfs.to_string_lossy().into_owned(),
        m.destination.display()
    );
    let dest = Path::new(&dest_for_host);

    let src = if m.typ == "bind" {
        let src = canonicalize(&m.source)?;
        let dir = if src.is_file() {
            Path::new(&dest).parent().unwrap()
        } else {
            Path::new(&dest)
        };
        create_dir_all(&dir).unwrap();
        if src.is_file() {
            OpenOptions::new()
                .create(true)
                .write(true)
                .open(&dest)
                .unwrap();
        }
        src
    } else {
        create_dir_all(&dest).unwrap();
        PathBuf::from(&m.source)
    };

    if let Err(::nix::Error::Sys(errno)) =
        nix_mount(Some(&*src), dest, Some(&*m.typ), flags, Some(&*d))
    {
        if errno != Errno::EINVAL {
            bail!("mount of {} failed", m.destination.display());
        }
        nix_mount(Some(&*src), dest, Some(&*m.typ), flags, Some(data))?;
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
        nix_mount(
            Some(&*dest),
            &*dest,
            None::<&str>,
            flags | MsFlags::MS_REMOUNT,
            None::<&str>,
        )?;
    }
    Ok(())
}

pub fn pivot_rootfs<P: ?Sized + NixPath>(path: &P) -> Result<()> {
    let newroot = open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;

    pivot_root(path, path)?;

    umount2("/", MntFlags::MNT_DETACH)?;
    fchdir(newroot)?;
    Ok(())
}

fn parse_mount(m: &Mount) -> (MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut data = Vec::new();
    for s in &m.options {
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
            "private" => Some((false, MsFlags::MS_PRIVATE)),
            "rprivate" => Some((false, MsFlags::MS_PRIVATE | MsFlags::MS_REC)),
            "shared" => Some((false, MsFlags::MS_SHARED)),
            "rshared" => Some((false, MsFlags::MS_SHARED | MsFlags::MS_REC)),
            "slave" => Some((false, MsFlags::MS_SLAVE)),
            "rslave" => Some((false, MsFlags::MS_SLAVE | MsFlags::MS_REC)),
            "relatime" => Some((false, MsFlags::MS_RELATIME)),
            "norelatime" => Some((true, MsFlags::MS_RELATIME)),
            "strictatime" => Some((false, MsFlags::MS_STRICTATIME)),
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
    (flags, data.join(","))
}
