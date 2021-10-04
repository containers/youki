use anyhow::Result;
use clap::Clap;
use nix;
use oci_spec::runtime::Mount;
use oci_spec::runtime::{
    LinuxBuilder, LinuxIdMappingBuilder, LinuxNamespace, LinuxNamespaceBuilder, LinuxNamespaceType,
    Spec,
};
use serde_json::to_writer_pretty;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
/// Command generates a config.json
#[derive(Clap, Debug)]
pub struct SpecJson {
    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}

pub fn get_default() -> Result<Spec> {
    Ok(Spec::default())
}

pub fn get_rootless() -> Result<Spec> {
    // Remove network and user namespace from the default spec
    let mut namespaces: Vec<LinuxNamespace> = oci_spec::runtime::get_default_namespaces()
        .into_iter()
        .filter(|ns| {
            ns.typ() != LinuxNamespaceType::Network && ns.typ() != LinuxNamespaceType::User
        })
        .collect();

    // Add user namespace
    namespaces.push(
        LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?,
    );

    let uid = nix::unistd::geteuid().as_raw();
    let gid = nix::unistd::getegid().as_raw();

    let linux = LinuxBuilder::default()
        .namespaces(namespaces)
        .uid_mappings(vec![LinuxIdMappingBuilder::default()
            .host_id(uid)
            .container_id(0_u32)
            .size(1_u32)
            .build()?])
        .gid_mappings(vec![LinuxIdMappingBuilder::default()
            .host_id(gid)
            .container_id(0_u32)
            .size(1_u32)
            .build()?])
        .build()?;

    // Prepare the mounts

    let mut mounts: Vec<Mount> = oci_spec::runtime::get_default_mounts();
    for mount in &mut mounts {
        if mount.destination().eq(Path::new("/sys")) {
            mount
                .set_source(Some(PathBuf::from("/sys")))
                .set_typ(Some(String::from("none")))
                .set_options(Some(vec![
                    "rbind".to_string(),
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                    "ro".to_string(),
                ]));
        } else {
            let options: Vec<String> = mount
                .options()
                .as_ref()
                .unwrap_or(&vec![])
                .iter()
                .filter(|&o| !o.starts_with("gid=") && !o.starts_with("uid="))
                .map(|o| o.to_string())
                .collect();
            mount.set_options(Some(options));
        }
    }

    let mut spec = get_default()?;
    spec.set_linux(Some(linux)).set_mounts(Some(mounts));
    Ok(spec)
}

/// spec Cli command
impl SpecJson {
    pub fn exec(&self) -> Result<()> {
        let spec = if self.rootless {
            get_rootless()?
        } else {
            get_default()?
        };

        // write data to config.json
        to_writer_pretty(&File::create("config.json")?, &spec)?;
        Ok(())
    }
}

#[cfg(test)]
// Tests become unstable if not serial. The cause is not known.
mod tests {
    use super::*;
    use crate::utils::create_temp_dir;

    #[test]
    fn test_spec_json() -> Result<()> {
        let spec = get_rootless()?;
        let tmpdir = create_temp_dir("test_spec_json").expect("failed to create temp dir");
        let path = tmpdir.path().join("config.json");
        to_writer_pretty(&File::create(path)?, &spec)?;
        Ok(())
    }
}
