use anyhow::Result;
use clap::Clap;
use nix;
use oci_spec::Spec;
use serde_json::to_writer_pretty;
use std::fs::File;
/// Command generates a config.json
#[derive(Clap, Debug)]
pub struct SpecJson {
    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}

pub fn set_for_rootless(spec: &mut Spec, uid: u32, gid: u32) -> Result<()> {
    let linux = spec.linux.as_mut().unwrap();
    linux.resources = None;

    // Remove network from the default spec
    let mut namespaces = vec![];
    for ns in linux.namespaces().as_ref().unwrap().iter() {
        if ns.typ() != LinuxNamespaceType::Network && ns.typ() != LinuxNamespaceType::User {
            namespaces.push(ns.clone());
        }
    }
    // Add user namespace
    namespaces.push(
        LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::User)
            .build()?,
    );
    linux.set_namespaces(Some(namespaces));

    linux.set_uid_mappings(Some(vec![LinuxIdMappingBuilder::default()
        .host_id(uid)
        .container_id(0_u32)
        .size(1_u32)
        .build()?]));
    linux.set_gid_mappings(Some(vec![LinuxIdMappingBuilder::default()
        .host_id(gid)
        .container_id(0_u32)
        .size(1_u32)
        .build()?]));

    // Fix the mounts
    let mut mounts = vec![];
    for mount in self.mounts().as_ref().unwrap().iter() {
        let dest = mount.destination().clone();
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
            let mut t = mount.clone();
            t.set_options(Some(options));
            mounts.push(t);
        }
    }
    self.set_mounts(Some(mounts));
    Ok(())
}

/// spec Cli command
impl SpecJson {
    pub fn exec(&self) -> Result<()> {
        // get default values for Spec
        let mut default_json: Spec = Default::default();
        if self.rootless {
            default_json.set_for_rootless(
                nix::unistd::geteuid().as_raw(),
                nix::unistd::getegid().as_raw(),
            )?;
        }
        // write data to config.json
        to_writer_pretty(&File::create("config.json")?, &default_json)?;
        Ok(())
    }
}
