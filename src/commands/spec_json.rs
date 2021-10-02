use anyhow::Result;
use clap::Clap;
use nix;
use oci_spec::runtime::{
    Linux, LinuxBuilder, LinuxIdMappingBuilder, LinuxNamespace, LinuxNamespaceBuilder,
    LinuxNamespaceType, MountBuilder, Spec, SpecBuilder,
};
use path_clean;
use serde_json::to_writer_pretty;
use std::fs::File;
use std::path::PathBuf;
/// Command generates a config.json
#[derive(Clap, Debug)]
pub struct SpecJson {
    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}

pub fn set_for_rootless(spec: &Spec) -> Result<Spec> {
    let uid = nix::unistd::geteuid().as_raw();
    let gid = nix::unistd::getegid().as_raw();

    // Remove network from the default spec
    let mut namespaces: Vec<LinuxNamespace> = spec
        .linux()
        .as_ref()
        .unwrap_or(&Linux::default())
        .namespaces()
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .filter(|&ns| {
            ns.typ() != LinuxNamespaceType::Network && ns.typ() != LinuxNamespaceType::User
        })
        .cloned()
        .collect();

    // Add user namespace
    namespaces.push(
        LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?,
    );
    let linux_builder = LinuxBuilder::default()
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
            .build()?]);

    // Fix the mounts
    let mut mounts = vec![];
    for mount in spec.mounts().as_ref().unwrap().iter() {
        let dest = mount.destination().clone();
        // Use path_clean to reduce multiple slashes to a single slash
        // and take care of '..' and '.' in dest path.
        if path_clean::clean(dest.as_path().to_str().unwrap()) == "/sys" {
            let mount = MountBuilder::default()
                .destination(PathBuf::from("/sys"))
                .source(PathBuf::from("/sys"))
                .typ("none".to_string())
                .options(vec![
                    "rbind".to_string(),
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                    "ro".to_string(),
                ])
                .build()?;
            mounts.push(mount);
        } else {
            let options: Vec<String> = mount
                .options()
                .as_ref()
                .unwrap_or(&vec![])
                .iter()
                .filter(|&o| !o.starts_with("gid=") && !o.starts_with("uid="))
                .map(|o| o.to_string())
                .collect();
            let mount_builder = MountBuilder::default().options(options);
            mounts.push(mount_builder.build()?);
        }
    }
    let spec_builder = SpecBuilder::default()
        .linux(linux_builder.build()?)
        .mounts(mounts);
    Ok(spec_builder.build()?)
}

/// spec Cli command
impl SpecJson {
    pub fn exec(&self) -> Result<()> {
        // get default values for Spec
        let mut default_json: Spec = Default::default();
        if self.rootless {
            default_json = set_for_rootless(&default_json)?
        };
        // write data to config.json
        to_writer_pretty(&File::create("config.json")?, &default_json)?;
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
        let mut spec = Default::default();
        spec = set_for_rootless(&spec)?;
        let tmpdir = create_temp_dir("test_spec_json").expect("failed to create temp dir");
        let path = tmpdir.path().join("config.json");
        to_writer_pretty(&File::create(path)?, &spec)?;
        Ok(())
    }
}
