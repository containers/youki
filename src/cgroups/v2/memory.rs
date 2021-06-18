use anyhow::{Result, *};
use std::path::Path;

use oci_spec::{LinuxMemory, LinuxResources};

use crate::cgroups::common;

use super::controller::Controller;

const CGROUP_MEMORY_SWAP: &str = "memory.swap.max";
const CGROUP_MEMORY_MAX: &str = "memory.max";
const CGROUP_MEMORY_LOW: &str = "memory.low";

pub struct Memory {}

impl Controller for Memory {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        if let Some(memory) = &linux_resources.memory {
            Self::apply(cgroup_path, memory)?;
        }

        Ok(())
    }
}

impl Memory {
    fn set<P: AsRef<Path>>(path: P, val: i64) -> Result<()> {
        if val == 0 {
            Ok(())
        } else if val == -1 {
            common::write_cgroup_file_str(path, "max")
        } else {
            common::write_cgroup_file(path, val)
        }
    }

    fn apply(path: &Path, memory: &LinuxMemory) -> Result<()> {
        // if nothing is set just exit right away
        if memory.reservation.is_none() && memory.limit.is_none() && memory.swap.is_none() {
            return Ok(());
        }

        match memory.limit {
            Some(limit) if limit < -1 => {
                bail!("invalid memory value: {}", limit);
            }
            Some(limit) => match memory.swap {
                Some(swap) if swap < -1 => {
                    bail!("invalid swap value: {}", swap);
                }
                Some(swap) => {
                    Memory::set(path.join(CGROUP_MEMORY_SWAP), swap)?;
                    Memory::set(path.join(CGROUP_MEMORY_MAX), limit)?;
                }
                None => {
                    if limit == -1 {
                        Memory::set(path.join(CGROUP_MEMORY_SWAP), -1)?;
                    }
                    Memory::set(path.join(CGROUP_MEMORY_MAX), limit)?;
                }
            },
            None => {
                if memory.swap.is_some() {
                    bail!("unsable to set swap limit without memory limit");
                }
            }
        };

        if let Some(reservation) = memory.reservation {
            if reservation < -1 {
                bail!("invalid memory reservation value: {}", reservation);
            }
            Memory::set(path.join(CGROUP_MEMORY_LOW), reservation)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::{create_temp_dir, set_fixture};
    use oci_spec::LinuxMemory;
    use std::fs::read_to_string;

    #[test]
    fn test_set_memory_v2() {
        let tmp = create_temp_dir("test_set_memory_v2").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

        let limit = 1024;
        let reservation = 512;
        let swap = 2048;
        let memory_limits = &LinuxMemory {
            limit: Some(limit),
            reservation: Some(reservation),
            swap: Some(swap),
            kernel: None,
            kernel_tcp: None,
            swappiness: None,
        };
        Memory::apply(&tmp, memory_limits).expect("apply memory limits");

        let limit_content = read_to_string(tmp.join(CGROUP_MEMORY_MAX)).expect("read memory limit");
        assert_eq!(limit_content, limit.to_string());

        let swap_content = read_to_string(tmp.join(CGROUP_MEMORY_SWAP)).expect("read swap limit");
        assert_eq!(swap_content, swap.to_string());

        let reservation_content =
            read_to_string(tmp.join(CGROUP_MEMORY_LOW)).expect("read memory reservation");
        assert_eq!(reservation_content, reservation.to_string());
    }

    #[test]
    fn test_set_memory_unlimited_v2() {
        let tmp = create_temp_dir("test_set_memory_unlimited_v2")
            .expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

        let memory_limits = &LinuxMemory {
            limit: Some(-1),
            reservation: None,
            swap: None,
            kernel: None,
            kernel_tcp: None,
            swappiness: None,
        };
        Memory::apply(&tmp, memory_limits).expect("apply memory limits");

        let limit_content = read_to_string(tmp.join(CGROUP_MEMORY_MAX)).expect("read memory limit");
        assert_eq!(limit_content, "max");

        let swap_content = read_to_string(tmp.join(CGROUP_MEMORY_SWAP)).expect("read swap limit");
        assert_eq!(swap_content, "max");
    }
}
