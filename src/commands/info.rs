//! Contains functions related to printing information about system running Youki
use std::{fs, path::Path};

use anyhow::Result;
use clap::Clap;
use procfs::{CpuInfo, Meminfo};

use cgroups;

#[derive(Clap, Debug)]
pub struct Info {}

impl Info {
    pub fn exec(&self) -> Result<()> {
        print_youki();
        print_kernel();
        print_os();
        print_hardware();
        print_cgroups();
        print_namespaces();

        Ok(())
    }
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

        if let (Some(name), Some(version)) = (name, version) {
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
    let param_value = content
        .lines()
        .find(|l| l.starts_with(param_name))
        .map(|l| l.split_terminator('=').last());

    if let Some(Some(value)) = param_value {
        return Some(value);
    }

    None
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
    if let Ok(cgroup_fs) = cgroups::common::get_supported_cgroup_fs() {
        let cgroup_fs: Vec<String> = cgroup_fs.into_iter().map(|c| c.to_string()).collect();
        println!("{:<18}{}", "Cgroup version", cgroup_fs.join(" and "));
    }

    println!("Cgroup mounts");
    if let Ok(v1_mounts) = cgroups::v1::util::list_subsystem_mount_points() {
        let mut v1_mounts: Vec<String> = v1_mounts
            .iter()
            .map(|kv| format!("  {:<16}{}", kv.0.to_string(), kv.1.display()))
            .collect();

        v1_mounts.sort();
        for cgroup_mount in v1_mounts {
            println!("{}", cgroup_mount);
        }
    }

    let unified = cgroups::v2::util::get_unified_mount_point();
    if let Ok(mount_point) = unified {
        println!("  {:<16}{}", "unified", mount_point.display());
    }
}

pub fn print_namespaces() {
    let uname = nix::sys::utsname::uname();
    let kernel_config = Path::new("/boot").join(format!("config-{}", uname.release()));
    if !kernel_config.exists() {
        return;
    }

    if let Ok(content) = fs::read_to_string(kernel_config) {
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
        print_feature_status(&content, "CONFIG_UTS_NS", "uts");
        print_feature_status(&content, "CONFIG_IPC_NS", "ipc");
        print_feature_status(&content, "CONFIG_USER_NS", "user");
        print_feature_status(&content, "CONFIG_PID_NS", "pid");
        print_feature_status(&content, "CONFIG_NET_NS", "network");
    }
}

fn print_feature_status(config: &str, feature: &str, display: &str) {
    if let Some(status_flag) = find_parameter(config, feature) {
        let status = if status_flag == "y" {
            "enabled"
        } else {
            "disabled"
        };

        println!("  {:<16}{}", display, status);
    } else {
        println!("  {:<16}disabled", display);
    }
}
