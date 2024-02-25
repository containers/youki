use super::ContainerData;
use anyhow::{bail, Result};
use libcgroups::v1::{
    util::{get_subsystem_mount_point, get_subsystem_path},
    ControllerType,
};
use oci_spec::runtime::{LinuxMemory, LinuxMemoryBuilder, Spec};

pub(crate) fn validate_linux_resource_memory(spec: &Spec, data: ContainerData) -> Result<()> {
    let expected_memory = spec
        .linux()
        .as_ref()
        .unwrap()
        .resources()
        .as_ref()
        .unwrap()
        .memory()
        .as_ref();
    let expected_memory = match expected_memory {
        Some(m) => m,
        None => bail!("expected memory to be set, but it was not"),
    };

    let memory = get_memory_data(data.state.unwrap().pid.unwrap());

    if let Err(e) = memory {
        bail!("failed to get memory data: {:?}", e);
    }

    let expected_limit = expected_memory.limit().unwrap();
    let actual_limit = memory.as_ref().unwrap().limit().unwrap();
    if expected_limit != actual_limit {
        bail!("expected memory limit {expected_limit}, but got {actual_limit} instead");
    }

    let expected_swappiness = expected_memory.swappiness().unwrap();
    let actual_swappiness = memory.as_ref().unwrap().swappiness().unwrap();
    if expected_memory.swappiness().unwrap() != actual_swappiness {
        bail!("expected memory swappiness {expected_swappiness}, got {actual_swappiness}");
    }

    Ok(())
}

fn get_memory_data(pid: i32) -> Result<LinuxMemory, Box<dyn std::error::Error>> {
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
                _ => unreachable!(),
            };
        }
    }
    Ok(memory_data.build()?)
}
