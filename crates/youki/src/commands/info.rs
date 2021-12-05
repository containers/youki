//! Contains functions related to printing information about system running Youki
use std::{collections::HashSet, fs, path::Path};

use anyhow::Result;
use clap::Parser;
use libcontainer::rootless;
use procfs::{CpuInfo, Meminfo};

use libcgroups::{common::CgroupSetup, v2::controller_type::ControllerType};

/// Show information about the system
#[derive(Parser, Debug)]
pub struct Info {}

pub fn info(_: Info) -> Result<()> {
    print_youki();
    print_kernel();
    print_os();
    print_hardware();
    print_cgroups();
    print_namespaces();

    Ok(())
}

/// print Version of Youki
pub fn print_youki() {
    println!("{:<18}{}", "Version", env!("CARGO_PKG_VERSION"));
}

/// Print Kernel Release, Version and Architecture
pub fn print_kernel() {
    let uname = nix::sys::utsname::uname();
    println!("{:<18}{}", "Kernel-Release", uname.release());
    println!("{:<18}{}", "Kernel-Version", uname.version());
    println!("{:<18}{}", "Architecture", uname.machine());
}

/// Prints OS Distribution information
// see https://www.freedesktop.org/software/systemd/man/os-release.html
pub fn print_os() {
    if let Some(os) = try_read_os_from("/etc/os-release") {
        println!("{:<18}{}", "Operating System", os);
    } else if let Some(os) = try_read_os_from("/usr/lib/os-release") {
        println!("{:<18}{}", "Operating System", os);
    }
}

/// Helper function to read the OS Distribution info
fn try_read_os_from<P: AsRef<Path>>(path: P) -> Option<String> {
    let os_release = path.as_ref();
    if !os_release.exists() {
        return None;
    }

    if let Ok(release_content) = fs::read_to_string(path) {
        let pretty = find_parameter(&release_content, "PRETTY_NAME");

        if let Some(pretty) = pretty {
            return Some(pretty.trim_matches('"').to_owned());
        }

        let name = find_parameter(&release_content, "NAME");
        let version = find_parameter(&release_content, "VERSION");

        if let Some((name, version)) = name.zip(version) {
            return Some(format!(
                "{} {}",
                name.trim_matches('"'),
                version.trim_matches('"')
            ));
        }
    }

    None
}

/// Helper function to find keyword values in OS info string
fn find_parameter<'a>(content: &'a str, param_name: &str) -> Option<&'a str> {
    content
        .lines()
        .find(|l| l.starts_with(param_name))
        .and_then(|l| l.split_terminator('=').last())
}

/// Print Hardware information of system
pub fn print_hardware() {
    if let Ok(cpu_info) = CpuInfo::new() {
        println!("{:<18}{}", "Cores", cpu_info.num_cores());
    }

    if let Ok(mem_info) = Meminfo::new() {
        println!(
            "{:<18}{}",
            "Total Memory",
            mem_info.mem_total / u64::pow(1024, 2)
        );
    }
}

/// Print cgroups info of system
pub fn print_cgroups() {
    let cgroup_setup = libcgroups::common::get_cgroup_setup();
    if let Ok(cgroup_setup) = &cgroup_setup {
        println!("{:<18}{}", "Cgroup setup", cgroup_setup);
    }

    println!("Cgroup mounts");
    if let Ok(v1_mounts) = libcgroups::v1::util::list_supported_mount_points() {
        let mut v1_mounts: Vec<String> = v1_mounts
            .iter()
            .map(|kv| format!("  {:<16}{}", kv.0.to_string(), kv.1.display()))
            .collect();

        v1_mounts.sort();
        for cgroup_mount in v1_mounts {
            println!("{}", cgroup_mount);
        }
    }

    let unified = libcgroups::v2::util::get_unified_mount_point();
    if let Ok(mount_point) = &unified {
        println!("  {:<16}{}", "unified", mount_point.display());
    }

    if let Ok(cgroup_setup) = cgroup_setup {
        if let Ok(unified) = &unified {
            if matches!(cgroup_setup, CgroupSetup::Hybrid | CgroupSetup::Unified) {
                if let Ok(controllers) = libcgroups::v2::util::get_available_controllers(unified) {
                    println!("CGroup v2 controllers");
                    let active_controllers: HashSet<ControllerType> =
                        controllers.into_iter().collect();
                    for controller in libcgroups::v2::controller_type::CONTROLLER_TYPES {
                        let status = if active_controllers.contains(controller) {
                            "attached"
                        } else {
                            "detached"
                        };

                        println!("  {:<16}{}", controller.to_string(), status);
                    }
                }

                if let Some(config) = read_kernel_config() {
                    let display = FeatureDisplay::with_status("device", "attached", "detached");
                    print_feature_status(&config, "CONFIG_CGROUP_BPF", display);
                }
            }
        }
    }
}

fn read_kernel_config() -> Option<String> {
    let uname = nix::sys::utsname::uname();
    let kernel_config = Path::new("/boot").join(format!("config-{}", uname.release()));
    if !kernel_config.exists() {
        return None;
    }

    fs::read_to_string(kernel_config).ok()
}

pub fn print_namespaces() {
    if let Some(content) = read_kernel_config() {
        if let Some(ns_enabled) = find_parameter(&content, "CONFIG_NAMESPACES") {
            if ns_enabled == "y" {
                println!("{:<18}enabled", "Namespaces");
            } else {
                println!("{:<18}disabled", "Namespaces");
                return;
            }
        }

        // mount namespace is always enabled if namespaces are enabled
        println!("  {:<16}enabled", "mount");
        print_feature_status(&content, "CONFIG_UTS_NS", FeatureDisplay::new("uts"));
        print_feature_status(&content, "CONFIG_IPC_NS", FeatureDisplay::new("ipc"));

        let user_display = match rootless::unprivileged_user_ns_enabled() {
            Ok(false) => FeatureDisplay::with_status("user", "enabled (root only)", "disabled"),
            _ => FeatureDisplay::new("user"),
        };
        print_feature_status(&content, "CONFIG_USER_NS", user_display);
        print_feature_status(&content, "CONFIG_PID_NS", FeatureDisplay::new("pid"));
        print_feature_status(&content, "CONFIG_NET_NS", FeatureDisplay::new("network"));
        // While the CONFIG_CGROUP_NS kernel feature exists, it is obsolete and should not be used. CGroup namespaces
        // are instead enabled with CONFIG_CGROUPS.
        print_feature_status(&content, "CONFIG_CGROUPS", FeatureDisplay::new("cgroup"))
    }
}

fn print_feature_status(config: &str, feature: &str, display: FeatureDisplay) {
    if let Some(status_flag) = find_parameter(config, feature) {
        let status = if status_flag == "y" {
            display.enabled
        } else {
            display.disabled
        };

        println!("  {:<16}{}", display.name, status);
    } else {
        println!("  {:<16}{}", display.name, display.disabled);
    }
}

struct FeatureDisplay<'a> {
    name: &'a str,
    enabled: &'a str,
    disabled: &'a str,
}

impl<'a> FeatureDisplay<'a> {
    fn new(name: &'a str) -> Self {
        Self {
            name,
            enabled: "enabled",
            disabled: "disabled",
        }
    }

    fn with_status(name: &'a str, enabled: &'a str, disabled: &'a str) -> Self {
        Self {
            name,
            enabled,
            disabled,
        }
    }
}
