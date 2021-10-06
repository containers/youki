use super::{
    symlink::Symlink,
    utils::{find_parent_mount, parse_mount},
};
use crate::syscall::{syscall::create_syscall, Syscall};
use crate::utils::PathBufExt;
use anyhow::{bail, Context, Result};
use cgroups::common::{CgroupSetup::{Hybrid, Legacy, Unified}, DEFAULT_CGROUP_ROOT};
use nix::{errno::Errno, mount::MsFlags};
use oci_spec::runtime::{Mount as SpecMount, MountBuilder as SpecMountBuilder};
use procfs::process::{MountOptFields, Process};
use std::{collections::HashMap, fs::{canonicalize, create_dir_all, OpenOptions}};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MountOptions<'a> {
    pub root: &'a Path,
    pub label: Option<&'a str>,
    pub cgroup_ns: bool,
}

pub struct Mount {
    syscall: Box<dyn Syscall>,
}

impl Default for Mount {
    fn default() -> Self {
        Self::new()
    }
}

impl Mount {
    pub fn new() -> Mount {
        Mount {
            syscall: create_syscall(),
        }
    }

    pub fn setup_mount(&self, mount: &SpecMount, options: &MountOptions) -> Result<()> {
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

    fn mount_cgroup_v1(&self, cgroup_mount: &SpecMount, options: &MountOptions) -> Result<()> {
        // create tmpfs into which the cgroup subsystems will be mounted
        let tmpfs = SpecMountBuilder::default()
            .source("tmpfs")
            .typ("tmpfs")
            .destination(cgroup_mount.destination())
            .options(
                ["noexec", "nosuid", "nodev", "mode=755"]
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<String>>(),
            )
            .build()
            .context("failed to build tmpfs for cgroup")?;
        self.setup_mount(&tmpfs, options).context("failed to mount tmpfs for cgroup")?;
    
        // get all cgroup mounts on the host system
        let host_mounts: Vec<PathBuf> = cgroups::v1::util::list_subsystem_mount_points()
            .context("failed to get subsystem mount points")?
            .into_iter()
            .filter(|p| p.as_path().starts_with(DEFAULT_CGROUP_ROOT))
            .collect();
    
        let cgroup_root = options
            .root
            .join_safely(cgroup_mount.destination())
            .context("could not join rootfs path with cgroup mount destination")?;

        let symlink = Symlink::new();
    
        // setup cgroup mounts for container
        for host_mount in &host_mounts {
            if let Some(subsystem_name) = host_mount.file_name().and_then(|n| n.to_str()) {
                if subsystem_name == "systemd" {
                    continue;
                }
    
                if options.cgroup_ns {
                    self.setup_namespaced_subsystem(cgroup_mount, options, subsystem_name)?;
                } else {
                    self.setup_emulated_subsystem(cgroup_mount, options, host_mount, subsystem_name)?;
                }
    
                symlink.setup_comount_symlinks(&cgroup_root, subsystem_name)?;
            } else {
                log::warn!("could not get subsystem name from {:?}", host_mount);
            }
        }
    
        Ok(())
    }

    // On some distros cgroup subsystems are comounted e.g. cpu,cpuacct or net_cls,net_prio. These systems
    // have to be comounted in the container as well as the kernel will reject trying to mount them separately.
    fn setup_namespaced_subsystem(&self,
        cgroup_mount: &SpecMount,
        options: &MountOptions,
        subsystem_name: &str,
    ) -> Result<()> {
        let subsystem_mount = SpecMountBuilder::default()
            .source("cgroup")
            .typ("cgroup")
            .destination(cgroup_mount.destination().join(subsystem_name))
            .options(
                ["noexec", "nosuid", "nodev"]
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<String>>(),
            )
            .build()
            .with_context(|| format!("failed to build {}", subsystem_name))?;
    
        log::debug!("Mounting cgroup subsystem: {:?}", subsystem_name);
        self.mount_to_container(
            &subsystem_mount,
            options.root,
            MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            subsystem_name,
            options.label,
        )
        .with_context(|| format!("failed to mount {:?}", subsystem_mount))
    }

    fn setup_emulated_subsystem(
        &self,
        mount: &SpecMount,
        options: &MountOptions,
        host_mount: &Path,
        subsystem_name: &str,
    ) -> Result<()> {
        let process_cgroups: HashMap<String, String> = Process::myself()?
            .cgroups()
            .context("failed to get process cgroups")?
            .into_iter()
            .map(|c| (c.controllers.join(","), c.pathname))
            .collect();
    
        log::debug!("{:?}", process_cgroups);
        if let Some(proc_path) = process_cgroups.get(subsystem_name) {
            let emulated = SpecMountBuilder::default()
                .source(
                    host_mount
                        .join_safely(proc_path.as_str())
                        .with_context(|| {
                            format!(
                                "failed to join mount source for {} subsystem",
                                subsystem_name
                            )
                        })?,
                )
                .destination(
                    mount
                        .destination()
                        .join_safely(subsystem_name)
                        .with_context(|| {
                            format!(
                                "failed to join mount destination for {} subsystem",
                                subsystem_name
                            )
                        })?,
                )
                .typ("bind")
                .options(
                    ["rw", "rbind"]
                        .iter()
                        .map(|o| o.to_string())
                        .collect::<Vec<String>>(),
                )
                .build()?;
            log::debug!("Mounting emulated cgroup subsystem: {:?}", emulated);
    
            self.setup_mount(&emulated, options)
                .with_context(|| format!("failed to mount {} cgroup hierarchy", subsystem_name))?;
        }
    
        Ok(())
    }

    fn mount_cgroup_v2(&self, _: &SpecMount, _: &MountOptions, _: MsFlags, _: &str) -> Result<()> {
        log::warn!("Mounting cgroup v2 is not yet supported");
        Ok(())
    }

    /// Make parent mount of rootfs private if it was shared, which is required by pivot_root.
    /// It also makes sure following bind mount does not propagate in other namespaces.
    pub fn make_parent_mount_private(&self, rootfs: &Path) -> Result<()> {
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

    fn mount_to_container(
        &self,
        m: &SpecMount,
        rootfs: &Path,
        flags: MsFlags,
        data: &str,
        label: Option<&str>,
    ) -> Result<()> {
        let typ = m.typ().as_deref();
        let mut d = data.to_string();

        if let Some(l) = label {
            if typ != Some("proc") && typ != Some("sysfs") {
                match data.is_empty() {
                    true => d = format!("context=\"{}\"", l),
                    false => d = format!("{},context=\"{}\"", data, l),
                }
            }
        }

        let dest_for_host = format!(
            "{}{}",
            rootfs.to_string_lossy().into_owned(),
            m.destination().display()
        );
        let dest = Path::new(&dest_for_host);
        let source = m
            .source()
            .as_ref()
            .with_context(|| "no source in mount spec".to_string())?;
        let src = if typ == Some("bind") {
            let src = canonicalize(source)
                .with_context(|| format!("Failed to canonicalize: {:?}", source))?;
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
                    .with_context(|| format!("Failed to create file for bind mount: {:?}", src))?;
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
                .mount(Some(&*src), dest, typ, flags, Some(data))
                .with_context(|| format!("Failed to mount {:?} to {:?}", src, dest))?;
        }

        if typ == Some("bind")
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
                .mount(Some(dest), dest, None, flags | MsFlags::MS_REMOUNT, None)
                .with_context(|| format!("Failed to remount: {:?}", dest))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::test::{MountArgs, TestHelperSyscall};
    use crate::utils::create_temp_dir;

    #[test]
    fn test_mount_to_container() {
        let tmp_dir = create_temp_dir("test_mount_to_container").unwrap();
        {
            let m = Mount::new();
            let mount = &SpecMountBuilder::default()
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
                .unwrap();
            let (flags, data) = parse_mount(mount);

            assert!(m
                .mount_to_container(mount, tmp_dir.path(), flags, &data, Some("defaults"))
                .is_ok());

            let want = vec![MountArgs {
                source: Some(PathBuf::from("devpts")),
                target: tmp_dir.path().join("dev/pts"),
                fstype: Some("devpts".to_string()),
                flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                data: Some(
                    "newinstance,ptmxmode=0666,mode=0620,gid=5,context=\"defaults\"".to_string(),
                ),
            }];
            let got = &m
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
            assert_eq!(want, *got);
            assert_eq!(got.len(), 1);
        }
        {
            let m = Mount::new();
            let mount = &SpecMountBuilder::default()
                .destination(PathBuf::from("/dev/null"))
                .typ("bind")
                .source(tmp_dir.path().join("null"))
                .options(vec!["ro".to_string()])
                .build()
                .unwrap();
            let (flags, data) = parse_mount(mount);
            OpenOptions::new()
                .create(true)
                .write(true)
                .open(tmp_dir.path().join("null"))
                .unwrap();

            assert!(m
                .mount_to_container(mount, tmp_dir.path(), flags, &data, None)
                .is_ok());

            let want = vec![
                MountArgs {
                    source: Some(tmp_dir.path().join("null")),
                    target: tmp_dir.path().join("dev/null"),
                    fstype: Some("bind".to_string()),
                    flags: MsFlags::MS_RDONLY,
                    data: Some("".to_string()),
                },
                // remount one
                MountArgs {
                    source: Some(tmp_dir.path().join("dev/null")),
                    target: tmp_dir.path().join("dev/null"),
                    fstype: None,
                    flags: MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
                    data: None,
                },
            ];
            let got = &m
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
            assert_eq!(want, *got);
            assert_eq!(got.len(), 2);
        }
    }

    #[test]
    fn test_make_parent_mount_private() {
        let tmp_dir = create_temp_dir("test_make_parent_mount_private").unwrap();
        let m = Mount::new();
        assert!(m.make_parent_mount_private(tmp_dir.path()).is_ok());

        let want = MountArgs {
            source: None,
            target: PathBuf::from("/"),
            fstype: None,
            flags: MsFlags::MS_PRIVATE,
            data: None,
        };
        let got = m
            .syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args();

        assert_eq!(got.len(), 1);
        assert_eq!(want, got[0]);
    }
}
