use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use oci_spec::runtime::{LinuxMemory, LinuxMemoryBuilder};
use procfs::{process::Process, ProcError};

use super::{controller_type::CONTROLLERS, ControllerType};

#[derive(thiserror::Error, Debug)]
pub enum V1MountPointError {
    #[error("failed to read process info from /proc/self: {0}")]
    ReadSelf(ProcError),
    #[error("failed to get mountinfo: {0}")]
    MountInfo(ProcError),
    #[error("could not find mountpoint for {subsystem}")]
    NotFound { subsystem: ControllerType },
}

/// List all cgroup v1 subsystem mount points on the system. This can include unsupported
/// subsystems, comounted controllers and named hierarchies.
pub fn list_subsystem_mount_points() -> Result<Vec<PathBuf>, V1MountPointError> {
    Ok(Process::myself()
        .map_err(V1MountPointError::ReadSelf)?
        .mountinfo()
        .map_err(V1MountPointError::MountInfo)?
        .into_iter()
        .filter(|m| m.fs_type == "cgroup")
        .map(|m| m.mount_point)
        .collect())
}

/// List the mount points of all currently supported cgroup subsystems.
pub fn list_supported_mount_points() -> Result<HashMap<ControllerType, PathBuf>, V1MountPointError>
{
    let mut mount_paths = HashMap::with_capacity(CONTROLLERS.len());

    for controller in CONTROLLERS {
        if let Ok(mount_point) = get_subsystem_mount_point(controller) {
            mount_paths.insert(controller.to_owned(), mount_point);
        }
    }

    Ok(mount_paths)
}

pub fn get_memory_data(pid: i32) -> Result<LinuxMemory, Box<dyn std::error::Error>> {
    let cgroup_mount_point = get_subsystem_mount_point(&ControllerType::Memory)?;
    let mut cgroup_path = get_subsystem_path(pid, "memory")?;

    // Removing the leading slash to convert the path to be relative to the cgroup mount point
    if cgroup_path.is_absolute() {
        cgroup_path = cgroup_path.strip_prefix("/")?.to_path_buf();
    }

    let mut memory_data = LinuxMemoryBuilder::default();
    let cgroup_memory_files = vec![
        "memory.limit_in_bytes",
        "memory.soft_limit_in_bytes",
        "memory.memsw.limit_in_bytes",
        "memory.kmem.limit_in_bytes",
        "memory.kmem.tcp.limit_in_bytes",
        "memory.swappiness",
        "memory.oom_control",
    ];

    let path = cgroup_mount_point.join(&cgroup_path);
    for file in cgroup_memory_files {
        let file_path = path.join(file);
        if file_path.exists() {
            let value = std::fs::read_to_string(&file_path)?;
            match file {
                "memory.limit_in_bytes" => {
                    let limit = value.trim().parse::<i64>()?;
                    memory_data = memory_data.limit(limit);
                }
                "memory.soft_limit_in_bytes" => {
                    let reservation = value.trim().parse::<i64>()?;
                    memory_data = memory_data.reservation(reservation);
                }
                "memory.memsw.limit_in_bytes" => {
                    let swap = value.trim().parse::<i64>()?;
                    memory_data = memory_data.swap(swap);
                }
                "memory.kmem.limit_in_bytes" => {
                    let kernel = value.trim().parse::<i64>()?;
                    memory_data = memory_data.kernel(kernel);
                }
                "memory.kmem.tcp.limit_in_bytes" => {
                    let kernel_tcp = value.trim().parse::<i64>()?;
                    memory_data = memory_data.kernel_tcp(kernel_tcp);
                }
                "memory.swappiness" => {
                    let swappiness = value.trim().parse::<u64>()?;
                    memory_data = memory_data.swappiness(swappiness);
                }
                "memory.oom_control" => {
                    let oom_control = value.split_whitespace().collect::<Vec<&str>>();
                    let oom_control = oom_control
                        .get(1)
                        .ok_or("Failed to get oom_control")?
                        .parse::<u64>()?;
                    memory_data = memory_data.disable_oom_killer(oom_control == 1);
                }
                _ => {}
            };
        }
    }
    Ok(memory_data.build()?)
}

pub fn get_subsystem_mount_point(subsystem: &ControllerType) -> Result<PathBuf, V1MountPointError> {
    let subsystem_name = subsystem.to_string();
    Process::myself()
        .map_err(V1MountPointError::ReadSelf)?
        .mountinfo()
        .map_err(V1MountPointError::MountInfo)?
        .into_iter()
        .find(|m| {
            if m.fs_type == "cgroup" {
                // Some systems mount net_prio and net_cls in the same directory
                // other systems mount them in their own directories. This
                // should handle both cases.
                if subsystem_name == "net_cls" {
                    return m.mount_point.ends_with("net_cls,net_prio")
                        || m.mount_point.ends_with("net_prio,net_cls")
                        || m.mount_point.ends_with("net_cls");
                } else if subsystem_name == "net_prio" {
                    return m.mount_point.ends_with("net_cls,net_prio")
                        || m.mount_point.ends_with("net_prio,net_cls")
                        || m.mount_point.ends_with("net_prio");
                }

                if subsystem_name == "cpu" {
                    return m.mount_point.ends_with("cpu,cpuacct")
                        || m.mount_point.ends_with("cpu");
                }
                if subsystem_name == "cpuacct" {
                    return m.mount_point.ends_with("cpu,cpuacct")
                        || m.mount_point.ends_with("cpuacct");
                }
            }
            m.mount_point.ends_with(&subsystem_name)
        })
        .map(|m| m.mount_point)
        .ok_or(V1MountPointError::NotFound {
            subsystem: *subsystem,
        })
}

pub fn get_subsystem_path(pid: i32, subsystem: &str) -> Result<PathBuf, io::Error> {
    let contents = fs::read_to_string(Path::new(&format!("/proc/{}/cgroup", pid)))
        .unwrap_or_else(|_| panic!("failed to read /proc/{}/cgroup", pid));
    for line in contents.lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let subparts: Vec<&str> = parts[1].split(',').collect();
        for subpart in subparts {
            if subpart == subsystem {
                return Ok(PathBuf::from(parts[2].to_string()));
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("subsystem {} not found", subsystem),
    ))
}
