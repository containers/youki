//! Contains Functionality of `features` container command
use std::path::Path;

use anyhow::Result;
use caps::CapSet;
use libcontainer::oci_spec::runtime::{
    version, ApparmorBuilder, CgroupBuilder, FeaturesBuilder, IDMapBuilder, IntelRdtBuilder,
    LinuxFeatureBuilder, LinuxNamespaceType, MountExtensionsBuilder, SelinuxBuilder,
};
use liboci_cli::Features;

// Function to query and return capabilities
fn query_caps() -> Result<Vec<String>> {
    let mut available_caps = Vec::new();

    for cap in caps::all() {
        // Check if the capability is in the permitted set
        if caps::has_cap(None, CapSet::Permitted, cap).unwrap_or(false) {
            available_caps.push(format!("{:?}", cap));
        }
    }

    Ok(available_caps)
}

// Function to query and return namespaces
fn query_supported_namespaces() -> Result<Vec<LinuxNamespaceType>> {
    let mut supported_namespaces = Vec::new();

    let ns_types = vec![
        LinuxNamespaceType::Pid,
        LinuxNamespaceType::Network,
        LinuxNamespaceType::Uts,
        LinuxNamespaceType::Ipc,
        LinuxNamespaceType::Mount,
        LinuxNamespaceType::User,
        LinuxNamespaceType::Cgroup,
        LinuxNamespaceType::Time,
    ];

    for ns in ns_types {
        let ns_path = format!("/proc/self/ns/{}", ns);
        if Path::new(&ns_path).exists() {
            supported_namespaces.push(ns);
        }
    }

    Ok(supported_namespaces)
}

fn known_hooks() -> Vec<String> {
    [
        "prestart",
        "createRuntime",
        "createContainer",
        "startContainer",
        "poststart",
        "poststop",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn known_mount_options() -> Vec<String> {
    [
        "acl",
        "async",
        "atime",
        "auto",
        "bind",
        "defaults",
        "dev",
        "dirsync",
        "exec",
        "mand",
        "noacl",
        "noatime",
        "nodev",
        "noexec",
        "nomand",
        "norelatime",
        "nosuid",
        "private",
        "rbind",
        "relatime",
        "remount",
        "ro",
        "rprivate",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// lists all existing containers
pub fn features(_: Features) -> Result<()> {
    // Query supported namespaces
    let namespaces = match query_supported_namespaces() {
        Ok(ns) => ns,
        Err(e) => {
            eprintln!("Error querying supported namespaces: {}", e);
            Vec::new()
        }
    };

    // Query available capabilities
    let capabilities = match query_caps() {
        Ok(caps) => caps,
        Err(e) => {
            eprintln!("Error querying available capabilities: {}", e);
            Vec::new()
        }
    };

    let linux = LinuxFeatureBuilder::default()
        .namespaces(namespaces)
        .capabilities(capabilities)
        .cgroup(
            CgroupBuilder::default()
                .v1(cfg!(feature = "v1"))
                .v2(cfg!(feature = "v2"))
                .systemd(cfg!(feature = "systemd"))
                .systemd_user(cfg!(feature = "systemd"))
                // cgroupv2 rdma controller is not implemented in youki.
                .rdma(false)
                .build()
                .unwrap(),
        )
        // TODO: Expose seccomp support information
        .apparmor(ApparmorBuilder::default().enabled(true).build().unwrap())
        .mount_extensions(
            MountExtensionsBuilder::default()
                // idmapped mounts is not supported in youki
                .idmap(IDMapBuilder::default().enabled(false).build().unwrap())
                .build()
                .unwrap(),
        )
        // SELinux is not supported in youki.
        .selinux(SelinuxBuilder::default().enabled(false).build().unwrap())
        .intel_rdt(IntelRdtBuilder::default().enabled(true).build().unwrap())
        .build()
        .unwrap();

    let features = FeaturesBuilder::default()
        .oci_version_max(version())
        .oci_version_min(String::from("1.0.0"))
        .hooks(known_hooks())
        .mount_options(known_mount_options())
        .linux(linux)
        .build()
        .unwrap();

    // Print out the created struct to verify
    let pretty_json_str = serde_json::to_string_pretty(&features)?;
    println!("{}", pretty_json_str);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_features() {
        let features = Features {};
        assert!(crate::commands::features::features(features).is_ok());
    }
}
