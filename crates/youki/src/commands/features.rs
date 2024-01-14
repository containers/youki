//! Contains Functionality of `features` container command
use anyhow::Result;
use caps::{all, CapSet};
use liboci_cli::Features;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const ANNOTATION_RUNC_VERSION: &str = "org.opencontainers.runc.version";
pub const ANNOTATION_RUNC_COMMIT: &str = "org.opencontainers.runc.commit";
pub const ANNOTATION_RUNC_CHECKPOINT_ENABLED: &str = "org.opencontainers.runc.checkpoint.enabled";
pub const ANNOTATION_LIBSECCOMP_VERSION: &str = "io.github.seccomp.libseccomp.version";

#[derive(Debug, Serialize, Deserialize)]
pub struct HardFeatures {
    // Minimum OCI Runtime Spec version recognized by the runtime, e.g., "1.0.0".
    oci_version_min: Option<String>,
    // Maximum OCI Runtime Spec version recognized by the runtime, e.g., "1.0.2-dev".
    oci_version_max: Option<String>,
    // List of the recognized hook names, e.g., "createRuntime".
    hooks: Option<Vec<String>>,
    // List of the recognized mount options, e.g., "ro".
    mount_options: Option<Vec<String>>,
    // Specific to Linux.
    linux: Option<Linux>,
    // Contains implementation-specific annotation strings.
    annotations: Option<std::collections::HashMap<String, String>>,
}

// Specific to Linux.
#[derive(Debug, Serialize, Deserialize)]
pub struct Linux {
    // List of the recognized namespaces, e.g., "mount".
    namespaces: Option<Vec<String>>,
    // List of the recognized capabilities, e.g., "CAP_SYS_ADMIN".
    capabilities: Option<Vec<String>>,
    cgroup: Option<Cgroup>,
    seccomp: Option<Seccomp>,
    apparmor: Option<Apparmor>,
    selinux: Option<Selinux>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Seccomp {
    enabled: Option<bool>,
    actions: Option<Vec<String>>,
    operators: Option<Vec<String>>,
    archs: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Apparmor {
    enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Selinux {
    enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Cgroup {
    v1: Option<bool>,
    v2: Option<bool>,
    systemd: Option<bool>,
    systemd_user: Option<bool>,
}

// Function to query and return capabilities
fn query_caps() -> Result<Vec<String>> {
    let mut available_caps = Vec::new();

    for cap in all() {
        // Check if the capability is in the permitted set
        if caps::has_cap(None, CapSet::Permitted, cap).unwrap_or(false) {
            available_caps.push(format!("{:?}", cap));
        }
    }

    Ok(available_caps)
}

// Function to query and return namespaces
fn query_supported_namespaces() -> Result<Vec<String>> {
    let mut supported_namespaces = Vec::new();

    let ns_types = vec!["pid", "net", "uts", "ipc", "mnt", "user", "cgroup", "time"];

    for ns in ns_types {
        let ns_path = format!("/proc/self/ns/{}", ns);
        if Path::new(&ns_path).exists() {
            supported_namespaces.push(ns.to_string());
        }
    }

    Ok(supported_namespaces)
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


    let features = HardFeatures {
        oci_version_min: Some(String::from("1.0.0")),
        oci_version_max: Some(String::from("1.0.2-dev")),
        hooks: Some(vec![
            String::from("prestart"),
            String::from("createRuntime"),
            String::from("createContainer"),
            String::from("startContainer"),
            String::from("poststart"),
            String::from("poststop"),
        ]),
        mount_options: Some(vec![
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
        ]),
        linux: Some(Linux {
            namespaces: Some(namespaces),
            capabilities: Some(capabilities),
            cgroup: Some(Cgroup {
                v1: Some(true),
                v2: Some(true),
                systemd: Some(true),
                systemd_user: Some(true),
            }),
            seccomp: Some(Seccomp {
                enabled: Some(true),
                actions: Some(vec![
                    String::from("SCMP_ACT_ALLOW"),
                    String::from("SCMP_ACT_ERRNO"),
                    String::from("SCMP_ACT_KILL"),
                    String::from("SCMP_ACT_KILL_PROCESS"),
                    String::from("SCMP_ACT_KILL_THREAD"),
                    String::from("SCMP_ACT_LOG"),
                    String::from("SCMP_ACT_NOTIFY"),
                    String::from("SCMP_ACT_TRACE"),
                    String::from("SCMP_ACT_TRAP"),
                ]),
                operators: Some(vec![
                    String::from("SCMP_CMP_EQ"),
                    String::from("SCMP_CMP_GE"),
                    String::from("SCMP_CMP_GT"),
                    String::from("SCMP_CMP_LE"),
                    String::from("SCMP_CMP_LT"),
                    String::from("SCMP_CMP_MASKED_EQ"),
                    String::from("SCMP_CMP_NE"),
                ]),
                archs: Some(vec![
                    String::from("SCMP_ARCH_AARCH64"),
                    String::from("SCMP_ARCH_ARM"),
                    String::from("SCMP_ARCH_MIPS"),
                    String::from("SCMP_ARCH_MIPS64"),
                    String::from("SCMP_ARCH_MIPS64N32"),
                    String::from("SCMP_ARCH_MIPSEL"),
                    String::from("SCMP_ARCH_MIPSEL64"),
                    String::from("SCMP_ARCH_MIPSEL64N32"),
                    String::from("SCMP_ARCH_PPC"),
                    String::from("SCMP_ARCH_PPC64"),
                    String::from("SCMP_ARCH_PPC64LE"),
                    String::from("SCMP_ARCH_RISCV64"),
                    String::from("SCMP_ARCH_S390"),
                    String::from("SCMP_ARCH_S390X"),
                    String::from("SCMP_ARCH_X32"),
                    String::from("SCMP_ARCH_X86"),
                    String::from("SCMP_ARCH_X86_64"),
                ]),
            }),
            apparmor: Some(Apparmor {
                enabled: Some(true),
            }),
            selinux: Some(Selinux {
                enabled: Some(true),
            }),
        }),
        annotations: {
            let mut annotations_map = HashMap::new();
            annotations_map.insert(
                ANNOTATION_LIBSECCOMP_VERSION.to_string(),
                String::from("2.5.3"),
            );
            annotations_map.insert(
                ANNOTATION_RUNC_CHECKPOINT_ENABLED.to_string(),
                String::from("true"),
            );
            annotations_map.insert(
                ANNOTATION_RUNC_COMMIT.to_string(),
                String::from("v1.1.9-0-gccaecfc"),
            );
            annotations_map.insert(ANNOTATION_RUNC_VERSION.to_string(), String::from("1.1.9"));
            Some(annotations_map)
        },
    };

    // Print out the created struct to verify
    let pretty_json_str = serde_json::to_string_pretty(&features)?;
    println!("{}", pretty_json_str);

    Ok(())
}
