use crate::{namespaces::Namespaces, utils};
use anyhow::{bail, Context, Result};
use libcgroups::common::rootless_required;
use nix::unistd::Pid;
use oci_spec::runtime::{Linux, LinuxIdMapping, LinuxNamespace, LinuxNamespaceType, Mount, Spec};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::{env, path::PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Rootless<'a> {
    /// Location of the newuidmap binary
    pub newuidmap: Option<PathBuf>,
    /// Location of the newgidmap binary
    pub newgidmap: Option<PathBuf>,
    /// Mappings for user ids
    pub(crate) uid_mappings: Option<&'a Vec<LinuxIdMapping>>,
    /// Mappings for group ids
    pub(crate) gid_mappings: Option<&'a Vec<LinuxIdMapping>>,
    /// Info on the user namespaces
    pub user_namespace: Option<LinuxNamespace>,
    /// Is rootless container requested by a privileged user
    pub privileged: bool,
}

impl<'a> Rootless<'a> {
    pub fn new(spec: &'a Spec) -> Result<Option<Rootless<'a>>> {
        let linux = spec.linux().as_ref().context("no linux in spec")?;
        let namespaces = Namespaces::from(linux.namespaces().as_ref());
        let user_namespace = namespaces.get(LinuxNamespaceType::User);

        // If conditions requires us to use rootless, we must either create a new
        // user namespace or enter an existing.
        if rootless_required() && user_namespace.is_none() {
            bail!("rootless container requires valid user namespace definition");
        }

        if user_namespace.is_some() && user_namespace.unwrap().path().is_none() {
            log::debug!("rootless container should be created");

            validate_spec_for_rootless(spec)
                .context("The spec failed to comply to rootless requirement")?;
            let mut rootless = Rootless::from(linux);
            if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
                rootless.newuidmap = Some(uid_binary);
                rootless.newgidmap = Some(gid_binary);
            }

            Ok(Some(rootless))
        } else {
            log::debug!("This is NOT a rootless container");
            Ok(None)
        }
    }

    pub fn write_uid_mapping(&self, target_pid: Pid) -> Result<()> {
        log::debug!("Write UID mapping for {:?}", target_pid);
        if let Some(uid_mappings) = self.uid_mappings {
            write_id_mapping(
                target_pid,
                get_uid_path(&target_pid).as_path(),
                uid_mappings,
                self.newuidmap.as_deref(),
            )
        } else {
            Ok(())
        }
    }

    pub fn write_gid_mapping(&self, target_pid: Pid) -> Result<()> {
        log::debug!("Write GID mapping for {:?}", target_pid);
        if let Some(gid_mappings) = self.gid_mappings {
            return write_id_mapping(
                target_pid,
                get_gid_path(&target_pid).as_path(),
                gid_mappings,
                self.newgidmap.as_deref(),
            );
        } else {
            Ok(())
        }
    }
}

impl<'a> From<&'a Linux> for Rootless<'a> {
    fn from(linux: &'a Linux) -> Self {
        let namespaces = Namespaces::from(linux.namespaces().as_ref());
        let user_namespace = namespaces.get(LinuxNamespaceType::User);
        Self {
            newuidmap: None,
            newgidmap: None,
            uid_mappings: linux.uid_mappings().as_ref(),
            gid_mappings: linux.gid_mappings().as_ref(),
            user_namespace: user_namespace.cloned(),
            privileged: nix::unistd::geteuid().is_root(),
        }
    }
}

#[cfg(not(test))]
fn get_uid_path(pid: &Pid) -> PathBuf {
    PathBuf::from(format!("/proc/{pid}/uid_map"))
}

#[cfg(test)]
pub fn get_uid_path(pid: &Pid) -> PathBuf {
    utils::get_temp_dir_path(format!("{pid}_mapping_path").as_str()).join("uid_map")
}

#[cfg(not(test))]
fn get_gid_path(pid: &Pid) -> PathBuf {
    PathBuf::from(format!("/proc/{pid}/gid_map"))
}

#[cfg(test)]
pub fn get_gid_path(pid: &Pid) -> PathBuf {
    utils::get_temp_dir_path(format!("{pid}_mapping_path").as_str()).join("gid_map")
}

pub fn unprivileged_user_ns_enabled() -> Result<bool> {
    let user_ns_sysctl = Path::new("/proc/sys/kernel/unprivileged_userns_clone");
    if !user_ns_sysctl.exists() {
        return Ok(true);
    }

    let content =
        fs::read_to_string(user_ns_sysctl).context("failed to read unprivileged userns clone")?;

    match content.trim().parse::<u8>()? {
        0 => Ok(false),
        1 => Ok(true),
        v => bail!("failed to parse unprivileged userns value: {}", v),
    }
}

/// Validates that the spec contains the required information for
/// running in rootless mode
fn validate_spec_for_rootless(spec: &Spec) -> Result<()> {
    let linux = spec.linux().as_ref().context("no linux in spec")?;
    let namespaces = Namespaces::from(linux.namespaces().as_ref());
    if namespaces.get(LinuxNamespaceType::User).is_none() {
        bail!("rootless containers require the specification of a user namespace");
    }

    let gid_mappings = linux
        .gid_mappings()
        .as_ref()
        .context("rootless containers require gidMappings in spec")?;
    let uid_mappings = linux
        .uid_mappings()
        .as_ref()
        .context("rootless containers require uidMappings in spec")?;

    if uid_mappings.is_empty() {
        bail!("rootless containers require at least one uid mapping");
    }
    if gid_mappings.is_empty() {
        bail!("rootless containers require at least one gid mapping")
    }

    validate_mounts_for_rootless(
        spec.mounts().as_ref().context("no mounts in spec")?,
        uid_mappings,
        gid_mappings,
    )?;

    if let Some(additional_gids) = spec
        .process()
        .as_ref()
        .and_then(|process| process.user().additional_gids().as_ref())
    {
        let privileged = nix::unistd::geteuid().is_root();

        match (privileged, additional_gids.is_empty()) {
            (true, false) => {
                for gid in additional_gids {
                    if !is_id_mapped(*gid, gid_mappings) {
                        bail!("gid {} is specified as supplementary group, but is not mapped in the user namespace", gid);
                    }
                }
            }
            (false, false) => {
                bail!(
                    "user is {} (unprivileged). Supplementary groups cannot be set in \
                        a rootless container for this user due to CVE-2014-8989",
                    nix::unistd::geteuid()
                )
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_mounts_for_rootless(
    mounts: &[Mount],
    uid_mappings: &[LinuxIdMapping],
    gid_mappings: &[LinuxIdMapping],
) -> Result<()> {
    for mount in mounts {
        if let Some(options) = mount.options() {
            for opt in options {
                if opt.starts_with("uid=") && !is_id_mapped(opt[4..].parse()?, uid_mappings) {
                    bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
                }

                if opt.starts_with("gid=") && !is_id_mapped(opt[4..].parse()?, gid_mappings) {
                    bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
                }
            }
        }
    }

    Ok(())
}

fn is_id_mapped(id: u32, mappings: &[LinuxIdMapping]) -> bool {
    mappings
        .iter()
        .any(|m| id >= m.container_id() && id <= m.container_id() + m.size())
}

/// Looks up the location of the newuidmap and newgidmap binaries which
/// are required to write multiple user/group mappings
pub fn lookup_map_binaries(spec: &Linux) -> Result<Option<(PathBuf, PathBuf)>> {
    if let Some(uid_mappings) = spec.uid_mappings() {
        if uid_mappings.len() == 1 && uid_mappings.len() == 1 {
            return Ok(None);
        }

        let uidmap = lookup_map_binary("newuidmap")?;
        let gidmap = lookup_map_binary("newgidmap")?;

        match (uidmap, gidmap) {
            (Some(newuidmap), Some(newgidmap)) => Ok(Some((newuidmap, newgidmap))),
            _ => bail!("newuidmap/newgidmap binaries could not be found in path. This is required if multiple id mappings are specified"),
        }
    } else {
        Ok(None)
    }
}

fn lookup_map_binary(binary: &str) -> Result<Option<PathBuf>> {
    let paths = env::var("PATH").context("could not find PATH")?;
    Ok(paths
        .split_terminator(':')
        .map(|p| Path::new(p).join(binary))
        .find(|p| p.exists()))
}

fn write_id_mapping(
    pid: Pid,
    map_file: &Path,
    mappings: &[LinuxIdMapping],
    map_binary: Option<&Path>,
) -> Result<()> {
    log::debug!("Write ID mapping: {:?}", mappings);

    match mappings.len() {
        0 => bail!("at least one id mapping needs to be defined"),
        1 => {
            let mapping = mappings
                .first()
                .and_then(|m| format!("{} {} {}", m.container_id(), m.host_id(), m.size()).into())
                .unwrap();
            utils::write_file(map_file, mapping)?;
        }
        _ => {
            let args: Vec<String> = mappings
                .iter()
                .flat_map(|m| {
                    [
                        m.container_id().to_string(),
                        m.host_id().to_string(),
                        m.size().to_string(),
                    ]
                })
                .collect();

            Command::new(map_binary.unwrap())
                .arg(pid.to_string())
                .args(args)
                .output()
                .with_context(|| format!("failed to execute {:?}", map_binary))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nix::unistd::getpid;
    use oci_spec::runtime::{
        LinuxBuilder, LinuxIdMappingBuilder, LinuxNamespaceBuilder, SpecBuilder,
    };
    use serial_test::serial;

    use crate::utils::{test_utils::gen_u32, TempDir};

    use super::*;

    #[test]
    fn test_validate_ok() -> Result<()> {
        let userns = LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?;
        let uid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(gen_u32())
            .container_id(0_u32)
            .size(10_u32)
            .build()?];
        let gid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(gen_u32())
            .container_id(0_u32)
            .size(10_u32)
            .build()?];
        let linux = LinuxBuilder::default()
            .namespaces(vec![userns])
            .uid_mappings(uid_mappings)
            .gid_mappings(gid_mappings)
            .build()?;
        let spec = SpecBuilder::default().linux(linux).build()?;
        assert!(validate_spec_for_rootless(&spec).is_ok());
        Ok(())
    }

    #[test]
    fn test_validate_err() -> Result<()> {
        let userns = LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?;
        let uid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(gen_u32())
            .container_id(0_u32)
            .size(10_u32)
            .build()?];
        let gid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(gen_u32())
            .container_id(0_u32)
            .size(10_u32)
            .build()?];

        let linux_no_userns = LinuxBuilder::default()
            .namespaces(vec![])
            .uid_mappings(uid_mappings.clone())
            .gid_mappings(gid_mappings.clone())
            .build()?;
        assert!(validate_spec_for_rootless(
            &SpecBuilder::default()
                .linux(linux_no_userns)
                .build()
                .unwrap()
        )
        .is_err());

        let linux_uid_empty = LinuxBuilder::default()
            .namespaces(vec![userns.clone()])
            .uid_mappings(vec![])
            .gid_mappings(gid_mappings.clone())
            .build()?;
        assert!(validate_spec_for_rootless(
            &SpecBuilder::default()
                .linux(linux_uid_empty)
                .build()
                .unwrap()
        )
        .is_err());

        let linux_gid_empty = LinuxBuilder::default()
            .namespaces(vec![userns.clone()])
            .uid_mappings(uid_mappings.clone())
            .gid_mappings(vec![])
            .build()?;
        assert!(validate_spec_for_rootless(
            &SpecBuilder::default()
                .linux(linux_gid_empty)
                .build()
                .unwrap()
        )
        .is_err());

        let linux_uid_none = LinuxBuilder::default()
            .namespaces(vec![userns.clone()])
            .gid_mappings(gid_mappings)
            .build()?;
        assert!(validate_spec_for_rootless(
            &SpecBuilder::default()
                .linux(linux_uid_none)
                .build()
                .unwrap()
        )
        .is_err());

        let linux_gid_none = LinuxBuilder::default()
            .namespaces(vec![userns])
            .uid_mappings(uid_mappings)
            .build()?;
        assert!(validate_spec_for_rootless(
            &SpecBuilder::default()
                .linux(linux_gid_none)
                .build()
                .unwrap()
        )
        .is_err());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_write_uid_mapping() -> Result<()> {
        let userns = LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?;
        let host_uid = gen_u32();
        let host_gid = gen_u32();
        let container_id = 0_u32;
        let size = 10_u32;
        let uid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(host_uid)
            .container_id(container_id)
            .size(size)
            .build()?];
        let gid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(host_gid)
            .container_id(container_id)
            .size(size)
            .build()?];
        let linux = LinuxBuilder::default()
            .namespaces(vec![userns])
            .uid_mappings(uid_mappings)
            .gid_mappings(gid_mappings)
            .build()?;
        let spec = SpecBuilder::default().linux(linux).build()?;
        let rootless = Rootless::new(&spec)?.unwrap();
        let pid = getpid();
        let tempdir = TempDir::new(get_uid_path(&pid).parent().unwrap())?;
        let uid_map_path = tempdir.join("uid_map");
        let _ = fs::File::create(&uid_map_path)?;
        rootless.write_uid_mapping(pid)?;
        assert_eq!(
            format!("{container_id} {host_uid} {size}"),
            fs::read_to_string(uid_map_path)?
        );
        rootless.write_gid_mapping(pid)?;
        Ok(())
    }

    #[test]
    #[serial]
    fn test_write_gid_mapping() -> Result<()> {
        let userns = LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?;
        let host_uid = gen_u32();
        let host_gid = gen_u32();
        let container_id = 0_u32;
        let size = 10_u32;
        let uid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(host_uid)
            .container_id(container_id)
            .size(size)
            .build()?];
        let gid_mappings = vec![LinuxIdMappingBuilder::default()
            .host_id(host_gid)
            .container_id(container_id)
            .size(size)
            .build()?];
        let linux = LinuxBuilder::default()
            .namespaces(vec![userns])
            .uid_mappings(uid_mappings)
            .gid_mappings(gid_mappings)
            .build()?;
        let spec = SpecBuilder::default().linux(linux).build()?;
        let rootless = Rootless::new(&spec)?.unwrap();
        let pid = getpid();
        let tempdir = TempDir::new(get_gid_path(&pid).parent().unwrap())?;
        let gid_map_path = tempdir.join("gid_map");
        let _ = fs::File::create(&gid_map_path)?;
        rootless.write_gid_mapping(pid)?;
        assert_eq!(
            format!("{container_id} {host_gid} {size}"),
            fs::read_to_string(gid_map_path)?
        );
        Ok(())
    }
}
