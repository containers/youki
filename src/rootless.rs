use std::{env, path::PathBuf};

use anyhow::{bail, Result};
use nix::sched::CloneFlags;
use oci_spec::{Linux, LinuxIdMapping, Mount, Spec};

use crate::namespaces::Namespaces;

#[derive(Debug, Clone)]
pub struct Rootless {
    /// Location of the newuidmap binary
    pub newuidmap: Option<PathBuf>,
    /// Location of the newgidmap binary
    pub newgidmap: Option<PathBuf>,
    /// Mappings for user ids
    pub uid_mappings: Vec<LinuxIdMapping>,
    /// Mappings for group ids
    pub gid_mappings: Vec<LinuxIdMapping>,
}

impl From<&Linux> for Rootless {
    fn from(linux: &Linux) -> Self {
        Self {
            newuidmap: None,
            newgidmap: None,
            uid_mappings: linux.uid_mappings.clone(),
            gid_mappings: linux.gid_mappings.clone(),
        }
    }
}

pub fn detect_rootless(spec: &Spec) -> Result<Option<Rootless>> {
    let linux = spec.linux.as_ref().unwrap();

    let rootless = if should_use_rootless() {
        log::debug!("rootless container should be created");
        log::warn!(
            "resource constraints and multi id mapping is unimplemented for rootless containers"
        );
        validate(spec)?;
        let mut rootless = Rootless::from(linux);
        if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
            rootless.newuidmap = Some(uid_binary);
            rootless.newgidmap = Some(gid_binary);
        }
        Some(rootless)
    } else {
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
pub fn validate(spec: &Spec) -> Result<()> {
    let linux = spec.linux.as_ref().unwrap();

    if linux.uid_mappings.is_empty() {
        bail!("rootless containers require at least one uid mapping");
    }

    if linux.gid_mappings.is_empty() {
        bail!("rootless containers require at least one gid mapping")
    }

    let namespaces: Namespaces = linux.namespaces.clone().into();
    if !namespaces.clone_flags.contains(CloneFlags::CLONE_NEWUSER) {
        bail!("rootless containers require the specification of a user namespace");
    }

    validate_mounts(&spec.mounts, &linux.uid_mappings, &linux.gid_mappings)?;

    Ok(())
}

fn validate_mounts(
    mounts: &[Mount],
    uid_mappings: &[LinuxIdMapping],
    gid_mappings: &[LinuxIdMapping],
) -> Result<()> {
    for mount in mounts {
        for opt in &mount.options {
            if opt.starts_with("uid=") && !is_id_mapped(&opt[4..], uid_mappings)? {
                bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
            }

            if opt.starts_with("gid=") && !is_id_mapped(&opt[4..], gid_mappings)? {
                bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
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
    if spec.uid_mappings.len() == 1 && spec.uid_mappings.len() == 1 {
        return Ok(None);
    }

    let uidmap = lookup_map_binary("newuidmap")?;
    let gidmap = lookup_map_binary("newgidmap")?;

    match (uidmap, gidmap) {
        (Some(newuidmap), Some(newgidmap)) => Ok(Some((newuidmap, newgidmap))),
        _ => bail!("newuidmap/newgidmap binaries could not be found in path. This is required if multiple id mappings are specified"),
    }
}

fn lookup_map_binary(binary: &str) -> Result<Option<PathBuf>> {
    let paths = env::var("PATH")?;
    Ok(paths
        .split_terminator(':')
        .find(|p| PathBuf::from(p).join(binary).exists())
        .map(PathBuf::from))
}
