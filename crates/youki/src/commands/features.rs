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
    vec![
        String::from("prestart"),
        String::from("createRuntime"),
        String::from("createContainer"),
        String::from("startContainer"),
        String::from("poststart"),
        String::from("poststop"),
    ]
}

fn known_mount_options() -> Vec<String> {
    vec![
        String::from("acl"),
        String::from("async"),
        String::from("atime"),
        String::from("bind"),
        String::from("defaults"),
        String::from("dev"),
        String::from("diratime"),
        String::from("dirsync"),
        String::from("exec"),
        String::from("iversion"),
        String::from("lazytime"),
        String::from("loud"),
        String::from("mand"),
        String::from("noacl"),
        String::from("noatime"),
        String::from("nodev"),
        String::from("nodiratime"),
        String::from("noexec"),
        String::from("noiversion"),
        String::from("nolazytime"),
        String::from("nomand"),
        String::from("norelatime"),
        String::from("nostrictatime"),
        String::from("nosuid"),
        String::from("nosymfollow"),
        String::from("private"),
        String::from("ratime"),
        String::from("rbind"),
        String::from("rdev"),
        String::from("rdiratime"),
        String::from("relatime"),
        String::from("remount"),
        String::from("rexec"),
        String::from("rnoatime"),
        String::from("rnodev"),
        String::from("rnodiratime"),
        String::from("rnoexec"),
        String::from("rnorelatime"),
        String::from("rnostrictatime"),
        String::from("rnosuid"),
        String::from("rnosymfollow"),
        String::from("ro"),
        String::from("rprivate"),
        String::from("rrelatime"),
        String::from("rro"),
        String::from("rrw"),
        String::from("rshared"),
        String::from("rslave"),
        String::from("rstrictatime"),
        String::from("rsuid"),
        String::from("rsymfollow"),
        String::from("runbindable"),
        String::from("rw"),
        String::from("shared"),
        String::from("silent"),
        String::from("slave"),
        String::from("strictatime"),
        String::from("suid"),
        String::from("symfollow"),
        String::from("sync"),
        String::from("tmpcopyup"),
        String::from("unbindable"),
    ]
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
