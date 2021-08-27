use crate::{namespaces::Namespaces, utils};
use anyhow::{bail, Context, Result};
use nix::unistd::Pid;
use oci_spec::{Linux, LinuxIdMapping, LinuxNamespace, LinuxNamespaceType, Mount, Spec};
use std::path::Path;
use std::process::Command;
use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Rootless<'a> {
    /// Location of the newuidmap binary
    pub newuidmap: Option<PathBuf>,
    /// Location of the newgidmap binary
    pub newgidmap: Option<PathBuf>,
    /// Mappings for user ids
    pub uid_mappings: Option<&'a Vec<LinuxIdMapping>>,
    /// Mappings for group ids
    pub gid_mappings: Option<&'a Vec<LinuxIdMapping>>,
    /// Info on the user namespaces
    user_namespace: Option<LinuxNamespace>,
}

impl<'a> From<&'a Linux> for Rootless<'a> {
    fn from(linux: &'a Linux) -> Self {
        let namespaces = Namespaces::from(linux.namespaces.as_ref());
        let user_namespace = namespaces.get(LinuxNamespaceType::User);
        Self {
            newuidmap: None,
            newgidmap: None,
            uid_mappings: linux.uid_mappings.as_ref(),
            gid_mappings: linux.gid_mappings.as_ref(),
            user_namespace: user_namespace.cloned(),
        }
    }
}

// If user namespace is detected, then we are going into rootless.
// If we are not root, check if we are user namespace.
pub fn detect_rootless(spec: &Spec) -> Result<Option<Rootless>> {
    let linux = spec.linux.as_ref().context("no linux in spec")?;
    let namespaces = Namespaces::from(linux.namespaces.as_ref());
    let user_namespace = namespaces.get(LinuxNamespaceType::User);
    // If conditions requires us to use rootless, we must either create a new
    // user namespace or enter an exsiting.
    if should_use_rootless() && user_namespace.is_none() {
        bail!("Rootless container requires valid user namespace definition");
    }

    // Go through rootless procedure only when entering into a new user namespace
    let rootless = if user_namespace.is_some() && user_namespace.unwrap().path.is_none() {
        log::debug!("rootless container should be created");
        log::warn!(
            "resource constraints and multi id mapping is unimplemented for rootless containers"
        );
        validate(spec).context("The spec failed to comply to rootless requirement")?;
        let linux = spec.linux.as_ref().context("no linux in spec")?;
        let mut rootless = Rootless::from(linux);
        if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
            rootless.newuidmap = Some(uid_binary);
            rootless.newgidmap = Some(gid_binary);
        }

        Some(rootless)
    } else {
        log::debug!("This is NOT a rootless container");
        None
    };

    Ok(rootless)
}

/// Checks if rootless mode should be used
pub fn should_use_rootless() -> bool {
    if !nix::unistd::geteuid().is_root() {
        return true;
    }

    if let Ok("true") = std::env::var("YOUKI_USE_ROOTLESS").as_deref() {
        return true;
    }

    false
}

/// Validates that the spec contains the required information for
/// running in rootless mode
fn validate(spec: &Spec) -> Result<()> {
    let linux = spec.linux.as_ref().context("no linux in spec")?;
    let namespaces = Namespaces::from(linux.namespaces.as_ref());
    if namespaces.get(LinuxNamespaceType::User).is_none() {
        bail!("rootless containers require the specification of a user namespace");
    }

    let gid_mappings = linux
        .gid_mappings
        .as_ref()
        .context("rootless containers require gid_mappings in spec")?;
    let uid_mappings = linux
        .uid_mappings
        .as_ref()
        .context("rootless containers require LinuxIdMapping in spec")?;

    if uid_mappings.is_empty() {
        bail!("rootless containers require at least one uid mapping");
    }

    if gid_mappings.is_empty() {
        bail!("rootless containers require at least one gid mapping")
    }

    validate_mounts(
        spec.mounts.as_ref().context("no mounts in spec")?,
        uid_mappings,
        gid_mappings,
    )?;

    Ok(())
}

fn validate_mounts(
    mounts: &[Mount],
    uid_mappings: &[LinuxIdMapping],
    gid_mappings: &[LinuxIdMapping],
) -> Result<()> {
    for mount in mounts {
        if let Some(options) = &mount.options {
            for opt in options {
                if opt.starts_with("uid=") && !is_id_mapped(&opt[4..], uid_mappings)? {
                    bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
                }

                if opt.starts_with("gid=") && !is_id_mapped(&opt[4..], gid_mappings)? {
                    bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
                }
            }
        }
    }

    Ok(())
}

fn is_id_mapped(id: &str, mappings: &[LinuxIdMapping]) -> Result<bool> {
    let id = id.parse::<u32>()?;
    Ok(mappings
        .iter()
        .any(|m| id >= m.container_id && id <= m.container_id + m.size))
}

/// Looks up the location of the newuidmap and newgidmap binaries which
/// are required to write multiple user/group mappings
pub fn lookup_map_binaries(spec: &Linux) -> Result<Option<(PathBuf, PathBuf)>> {
    if let Some(uid_mappings) = spec.uid_mappings.as_ref() {
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
    let paths = env::var("PATH")?;
    Ok(paths
        .split_terminator(':')
        .find(|p| PathBuf::from(p).join(binary).exists())
        .map(PathBuf::from))
}

pub fn write_uid_mapping(target_pid: Pid, rootless: Option<&Rootless>) -> Result<()> {
    log::debug!("Write UID mapping for {:?}", target_pid);
    if let Some(rootless) = rootless {
        if let Some(uid_mappings) = rootless.gid_mappings {
            return write_id_mapping(
                &format!("/proc/{}/uid_map", target_pid),
                uid_mappings,
                rootless.newuidmap.as_deref(),
            );
        }
    }

    Ok(())
}

pub fn write_gid_mapping(target_pid: Pid, rootless: Option<&Rootless>) -> Result<()> {
    log::debug!("Write GID mapping for {:?}", target_pid);
    if let Some(rootless) = rootless {
        if let Some(gid_mappings) = rootless.gid_mappings {
            return write_id_mapping(
                &format!("/proc/{}/gid_map", target_pid),
                gid_mappings,
                rootless.newgidmap.as_deref(),
            );
        }
    }

    Ok(())
}

fn write_id_mapping(
    map_file: &str,
    mappings: &[oci_spec::LinuxIdMapping],
    map_binary: Option<&Path>,
) -> Result<()> {
    let mappings: Vec<String> = mappings
        .iter()
        .map(|m| format!("{} {} {}", m.container_id, m.host_id, m.size))
        .collect();
    log::debug!("Write ID mapping: {:?}", mappings);
    if mappings.len() == 1 {
        utils::write_file(map_file, mappings.first().unwrap())?;
    } else {
        Command::new(map_binary.unwrap())
            .args(mappings)
            .output()
            .with_context(|| format!("failed to execute {:?}", map_binary))?;
    }

    Ok(())
}
