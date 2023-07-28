use crate::error::MissingSpecError;
use crate::namespaces::{NamespaceError, Namespaces};
use nix::unistd::Pid;
use oci_spec::runtime::{Linux, LinuxIdMapping, LinuxNamespace, LinuxNamespaceType, Mount, Spec};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::{env, path::PathBuf};

// Wrap the uid/gid path function into a struct for dependency injection. This
// allows us to mock the id mapping logic in unit tests by using a different
// base path other than `/proc`.
#[derive(Debug, Clone)]
pub struct RootlessIDMapper {
    base_path: PathBuf,
}

impl Default for RootlessIDMapper {
    fn default() -> Self {
        Self {
            // By default, the `uid_map` and `gid_map` files are located in the
            // `/proc` directory. In the production code, we can use the
            // default.
            base_path: PathBuf::from("/proc"),
        }
    }
}

impl RootlessIDMapper {
    // In production code, we can direclt use the `new` function without the
    // need to worry about the default.
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_uid_path(&self, pid: &Pid) -> PathBuf {
        self.base_path.join(pid.to_string()).join("uid_map")
    }
    pub fn get_gid_path(&self, pid: &Pid) -> PathBuf {
        self.base_path.join(pid.to_string()).join("gid_map")
    }

    #[cfg(test)]
    pub fn ensure_uid_path(&self, pid: &Pid) -> std::result::Result<(), std::io::Error> {
        std::fs::create_dir_all(self.get_uid_path(pid).parent().unwrap())?;

        Ok(())
    }

    #[cfg(test)]
    pub fn ensure_gid_path(&self, pid: &Pid) -> std::result::Result<(), std::io::Error> {
        std::fs::create_dir_all(self.get_gid_path(pid).parent().unwrap())?;

        Ok(())
    }

    #[cfg(test)]
    // In test, we need to fake the base path to a temporary directory.
    pub fn new_test(path: PathBuf) -> Self {
        Self { base_path: path }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RootlessError {
    #[error(transparent)]
    MissingSpec(#[from] crate::error::MissingSpecError),
    #[error("rootless container requires valid user namespace definition")]
    NoUserNamespace,
    #[error("invalid spec for rootless container")]
    InvalidSpec(#[from] ValidateSpecError),
    #[error("failed to read unprivileged userns clone")]
    ReadUnprivilegedUsernsClone(#[source] std::io::Error),
    #[error("failed to parse unprivileged userns clone")]
    ParseUnprivilegedUsernsClone(#[source] std::num::ParseIntError),
    #[error("unknown userns clone value")]
    UnknownUnprivilegedUsernsClone(u8),
    #[error(transparent)]
    IDMapping(#[from] MappingError),
}

type Result<T> = std::result::Result<T, RootlessError>;

#[derive(Debug, thiserror::Error)]
pub enum ValidateSpecError {
    #[error(transparent)]
    MissingSpec(#[from] crate::error::MissingSpecError),
    #[error("rootless container requires valid user namespace definition")]
    NoUserNamespace,
    #[error("rootless container requires valid uid mappings")]
    NoUIDMappings,
    #[error("rootless container requires valid gid mappings")]
    NoGIDMapping,
    #[error("no mount in spec")]
    NoMountSpec,
    #[error("unprivileged user can't set supplementary groups")]
    UnprivilegedUser,
    #[error("supplementary group needs to be mapped in the gid mappings")]
    GidNotMapped(u32),
    #[error("failed to parse ID")]
    ParseID(#[source] std::num::ParseIntError),
    #[error("mount options require mapping uid inside the rootless container")]
    MountGidMapping(u32),
    #[error("mount options require mapping gid inside the rootless container")]
    MountUidMapping(u32),
    #[error(transparent)]
    Namespaces(#[from] NamespaceError),
}

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("newuidmap/newgidmap binaries could not be found in path")]
    BinaryNotFound,
    #[error("could not find PATH")]
    NoPathEnv,
    #[error("failed to execute newuidmap/newgidmap")]
    Execute(#[source] std::io::Error),
    #[error("at least one id mapping needs to be defined")]
    NoIDMapping,
    #[error("failed to write id mapping")]
    WriteIDMapping(#[source] std::io::Error),
}

#[derive(Debug, Clone, Default)]
pub struct Rootless {
    /// Location of the newuidmap binary
    pub newuidmap: Option<PathBuf>,
    /// Location of the newgidmap binary
    pub newgidmap: Option<PathBuf>,
    /// Mappings for user ids
    pub(crate) uid_mappings: Option<Vec<LinuxIdMapping>>,
    /// Mappings for group ids
    pub(crate) gid_mappings: Option<Vec<LinuxIdMapping>>,
    /// Info on the user namespaces
    pub user_namespace: Option<LinuxNamespace>,
    /// Is rootless container requested by a privileged user
    pub privileged: bool,
    /// Path to the id mappings
    pub rootless_id_mapper: RootlessIDMapper,
}

impl Rootless {
    pub fn new(spec: &Spec) -> Result<Option<Rootless>> {
        let linux = spec.linux().as_ref().ok_or(MissingSpecError::Linux)?;
        let namespaces = Namespaces::try_from(linux.namespaces().as_ref())
            .map_err(ValidateSpecError::Namespaces)?;
        let user_namespace = namespaces
            .get(LinuxNamespaceType::User)
            .map_err(ValidateSpecError::Namespaces)?;

        // If conditions requires us to use rootless, we must either create a new
        // user namespace or enter an existing.
        if rootless_required() && user_namespace.is_none() {
            return Err(RootlessError::NoUserNamespace);
        }

        if user_namespace.is_some() && user_namespace.unwrap().path().is_none() {
            tracing::debug!("rootless container should be created");

            validate_spec_for_rootless(spec).map_err(|err| {
                tracing::error!("failed to validate spec for rootless container: {}", err);
                err
            })?;
            let mut rootless = Rootless::try_from(linux)?;
            if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
                rootless.newuidmap = Some(uid_binary);
                rootless.newgidmap = Some(gid_binary);
            }

            Ok(Some(rootless))
        } else {
            tracing::debug!("this is NOT a rootless container");
            Ok(None)
        }
    }

    pub fn write_uid_mapping(&self, target_pid: Pid) -> Result<()> {
        tracing::debug!("write UID mapping for {:?}", target_pid);
        if let Some(uid_mappings) = self.uid_mappings.as_ref() {
            write_id_mapping(
                target_pid,
                self.rootless_id_mapper.get_uid_path(&target_pid).as_path(),
                uid_mappings,
                self.newuidmap.as_deref(),
            )?;
        }
        Ok(())
    }

    pub fn write_gid_mapping(&self, target_pid: Pid) -> Result<()> {
        tracing::debug!("write GID mapping for {:?}", target_pid);
        if let Some(gid_mappings) = self.gid_mappings.as_ref() {
            write_id_mapping(
                target_pid,
                self.rootless_id_mapper.get_gid_path(&target_pid).as_path(),
                gid_mappings,
                self.newgidmap.as_deref(),
            )?;
        }
        Ok(())
    }

    pub fn with_id_mapper(&mut self, mapper: RootlessIDMapper) {
        self.rootless_id_mapper = mapper
    }
}

impl TryFrom<&Linux> for Rootless {
    type Error = RootlessError;

    fn try_from(linux: &Linux) -> Result<Self> {
        let namespaces = Namespaces::try_from(linux.namespaces().as_ref())
            .map_err(ValidateSpecError::Namespaces)?;
        let user_namespace = namespaces
            .get(LinuxNamespaceType::User)
            .map_err(ValidateSpecError::Namespaces)?;
        Ok(Self {
            newuidmap: None,
            newgidmap: None,
            uid_mappings: linux.uid_mappings().to_owned(),
            gid_mappings: linux.gid_mappings().to_owned(),
            user_namespace: user_namespace.cloned(),
            privileged: nix::unistd::geteuid().is_root(),
            rootless_id_mapper: RootlessIDMapper::new(),
        })
    }
}

/// Checks if rootless mode should be used
pub fn rootless_required() -> bool {
    if !nix::unistd::geteuid().is_root() {
        return true;
    }

    matches!(std::env::var("YOUKI_USE_ROOTLESS").as_deref(), Ok("true"))
}

pub fn unprivileged_user_ns_enabled() -> Result<bool> {
    let user_ns_sysctl = Path::new("/proc/sys/kernel/unprivileged_userns_clone");
    if !user_ns_sysctl.exists() {
        return Ok(true);
    }

    let content =
        fs::read_to_string(user_ns_sysctl).map_err(RootlessError::ReadUnprivilegedUsernsClone)?;

    match content
        .trim()
        .parse::<u8>()
        .map_err(RootlessError::ParseUnprivilegedUsernsClone)?
    {
        0 => Ok(false),
        1 => Ok(true),
        v => Err(RootlessError::UnknownUnprivilegedUsernsClone(v)),
    }
}

/// Validates that the spec contains the required information for
/// running in rootless mode
fn validate_spec_for_rootless(spec: &Spec) -> std::result::Result<(), ValidateSpecError> {
    tracing::debug!(?spec, "validating spec for rootless container");
    let linux = spec.linux().as_ref().ok_or(MissingSpecError::Linux)?;
    let namespaces = Namespaces::try_from(linux.namespaces().as_ref())?;
    if namespaces.get(LinuxNamespaceType::User)?.is_none() {
        return Err(ValidateSpecError::NoUserNamespace);
    }

    let gid_mappings = linux
        .gid_mappings()
        .as_ref()
        .ok_or(ValidateSpecError::NoGIDMapping)?;
    let uid_mappings = linux
        .uid_mappings()
        .as_ref()
        .ok_or(ValidateSpecError::NoUIDMappings)?;

    if uid_mappings.is_empty() {
        return Err(ValidateSpecError::NoUIDMappings);
    }
    if gid_mappings.is_empty() {
        return Err(ValidateSpecError::NoGIDMapping);
    }

    validate_mounts_for_rootless(
        spec.mounts()
            .as_ref()
            .ok_or(ValidateSpecError::NoMountSpec)?,
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
                        tracing::error!(?gid,"gid is specified as supplementary group, but is not mapped in the user namespace");
                        return Err(ValidateSpecError::GidNotMapped(*gid));
                    }
                }
            }
            (false, false) => {
                tracing::error!(
                    user = ?nix::unistd::geteuid(),
                    "user is unprivileged. Supplementary groups cannot be set in \
                        a rootless container for this user due to CVE-2014-8989",
                );
                return Err(ValidateSpecError::UnprivilegedUser);
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
) -> std::result::Result<(), ValidateSpecError> {
    for mount in mounts {
        if let Some(options) = mount.options() {
            for opt in options {
                if opt.starts_with("uid=")
                    && !is_id_mapped(
                        opt[4..].parse().map_err(ValidateSpecError::ParseID)?,
                        uid_mappings,
                    )
                {
                    tracing::error!(
                        ?mount,
                        ?opt,
                        "mount specifies option which is not mapped inside the rootless container"
                    );
                    return Err(ValidateSpecError::MountUidMapping(
                        opt[4..].parse().map_err(ValidateSpecError::ParseID)?,
                    ));
                }

                if opt.starts_with("gid=")
                    && !is_id_mapped(
                        opt[4..].parse().map_err(ValidateSpecError::ParseID)?,
                        gid_mappings,
                    )
                {
                    tracing::error!(
                        ?mount,
                        ?opt,
                        "mount specifies option which is not mapped inside the rootless container"
                    );
                    return Err(ValidateSpecError::MountGidMapping(
                        opt[4..].parse().map_err(ValidateSpecError::ParseID)?,
                    ));
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
pub fn lookup_map_binaries(
    spec: &Linux,
) -> std::result::Result<Option<(PathBuf, PathBuf)>, MappingError> {
    if let Some(uid_mappings) = spec.uid_mappings() {
        if uid_mappings.len() == 1 && uid_mappings.len() == 1 {
            return Ok(None);
        }

        let uidmap = lookup_map_binary("newuidmap")?;
        let gidmap = lookup_map_binary("newgidmap")?;

        match (uidmap, gidmap) {
            (Some(newuidmap), Some(newgidmap)) => Ok(Some((newuidmap, newgidmap))),
            _ => Err(MappingError::BinaryNotFound),
        }
    } else {
        Ok(None)
    }
}

fn lookup_map_binary(binary: &str) -> std::result::Result<Option<PathBuf>, MappingError> {
    let paths = env::var("PATH").map_err(|_| MappingError::NoPathEnv)?;
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
) -> std::result::Result<(), MappingError> {
    tracing::debug!("Write ID mapping: {:?}", mappings);

    match mappings.len() {
        0 => return Err(MappingError::NoIDMapping),
        1 => {
            let mapping = mappings
                .first()
                .and_then(|m| format!("{} {} {}", m.container_id(), m.host_id(), m.size()).into())
                .unwrap();
            std::fs::write(map_file, &mapping).map_err(|err| {
                tracing::error!(?err, ?map_file, ?mapping, "failed to write uid/gid mapping");
                MappingError::WriteIDMapping(err)
            })?;
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
                .map_err(|err| {
                    tracing::error!(?err, ?map_binary, "failed to execute newuidmap/newgidmap");
                    MappingError::Execute(err)
                })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use anyhow::Result;
    use nix::unistd::getpid;
    use oci_spec::runtime::{
        LinuxBuilder, LinuxIdMappingBuilder, LinuxNamespaceBuilder, SpecBuilder,
    };
    use rand::Rng;
    use serial_test::serial;

    fn gen_u32() -> u32 {
        rand::thread_rng().gen()
    }

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

        let pid = getpid();
        let tmp = tempfile::tempdir()?;
        let id_mapper = RootlessIDMapper {
            base_path: tmp.path().to_path_buf(),
        };
        id_mapper.ensure_uid_path(&pid)?;

        let mut rootless = Rootless::new(&spec)?.unwrap();
        rootless.with_id_mapper(id_mapper.clone());
        rootless.write_uid_mapping(pid)?;
        assert_eq!(
            format!("{container_id} {host_uid} {size}"),
            fs::read_to_string(id_mapper.get_uid_path(&pid))?
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

        let pid = getpid();
        let tmp = tempfile::tempdir()?;
        let id_mapper = RootlessIDMapper {
            base_path: tmp.path().to_path_buf(),
        };
        id_mapper.ensure_gid_path(&pid)?;

        let mut rootless = Rootless::new(&spec)?.unwrap();
        rootless.with_id_mapper(id_mapper.clone());
        rootless.write_gid_mapping(pid)?;
        assert_eq!(
            format!("{container_id} {host_gid} {size}"),
            fs::read_to_string(id_mapper.get_gid_path(&pid))?
        );
        Ok(())
    }
}
