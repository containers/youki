use anyhow::{bail, Result};
use nix::sched::CloneFlags;
use oci_spec::{LinuxIdMapping, Mount, Spec};

use crate::namespaces::Namespaces;

/// Checks if rootless mode should be used
pub fn should_use_rootless() -> Result<bool> {
    if !nix::unistd::geteuid().is_root() {
        return Ok(true);
    }

    if let Ok("true") = std::env::var("YOUKI_USE_ROOTLESS").as_deref() {
        return Ok(true);
    }

    Ok(false)
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
    mounts: &Vec<Mount>,
    uid_mappings: &Vec<LinuxIdMapping>,
    gid_mappings: &Vec<LinuxIdMapping>,
) -> Result<()> {
    for mount in mounts {
        for opt in &mount.options {
            if opt.starts_with("uid=") && !is_id_mapped(&opt[4..], uid_mappings)? {
                bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
            } else if opt.starts_with("gid=") && !is_id_mapped(&opt[4..], gid_mappings)? {
                bail!("Mount {:?} specifies option {} which is not mapped inside the rootless container", mount, opt);
            }
        }
    }

    Ok(())
}

fn is_id_mapped(id: &str, mappings: &Vec<LinuxIdMapping>) -> Result<bool> {
    let id = id.parse::<u32>()?;
    Ok(mappings
        .iter()
        .all(|m| id >= m.container_id && id <= m.container_id + m.size))
}