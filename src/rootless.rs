use crate::{namespaces::Namespaces, utils};
use anyhow::{bail, Context, Result};
use nix::unistd::Pid;
use oci_spec::runtime::{Linux, LinuxIdMapping, LinuxNamespace, LinuxNamespaceType, Mount, Spec};
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
    pub(crate) user_namespace: Option<LinuxNamespace>,
    /// Is rootless container requested by a privileged user
    pub privileged: bool,
}

impl<'a> Rootless<'a> {
    pub fn new(spec: &'a Spec) -> Result<Option<Rootless<'a>>> {
        let linux = spec.linux().as_ref().context("no linux in spec")?;
        let namespaces = Namespaces::from(linux.namespaces().as_ref());
        let user_namespace = namespaces.get(LinuxNamespaceType::User);

        // If conditions requires us to use rootless, we must either create a new
        // user namespace or enter an exsiting.
        if rootless_required() && user_namespace.is_none() {
            bail!("rootless container requires valid user namespace definition");
        }

        if user_namespace.is_some() && user_namespace.unwrap().path().is_none() {
            log::debug!("rootless container should be created");
            log::warn!(
                "resource constraints and multi id mapping is unimplemented for rootless containers"
            );

            validate(spec).context("The spec failed to comply to rootless requirement")?;
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
                &format!("/proc/{}/uid_map", target_pid),
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
                &format!("/proc/{}/gid_map", target_pid),
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

/// Checks if rootless mode should be used
pub fn rootless_required() -> bool {
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

    validate_mounts(
        spec.mounts().as_ref().context("no mounts in spec")?,
        uid_mappings,
        gid_mappings,
    )?;

    if let Some(process) = &spec.process() {
        if let Some(additional_gids) = &process.user().additional_gids() {
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
    }

    Ok(())
}

fn validate_mounts(
    mounts: &[Mount],
    uid_mappings: &[LinuxIdMapping],
    gid_mappings: &[LinuxIdMapping],
) -> Result<()> {
    for mount in mounts {
        if let Some(options) = &mount.options() {
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
    if let Some(uid_mappings) = spec.uid_mappings().as_ref() {
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

fn write_id_mapping(
    map_file: &str,
    mappings: &[LinuxIdMapping],
    map_binary: Option<&Path>,
) -> Result<()> {
    let mappings: Vec<String> = mappings
        .iter()
        .map(|m| format!("{} {} {}", m.container_id(), m.host_id(), m.size()))
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
