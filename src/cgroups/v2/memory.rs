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
                    bail!("unable to set swap limit without memory limit");
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
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
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

    #[test]
    fn test_err_swap_no_memory_v2() {
        let tmp =
            create_temp_dir("test_err_swap_no_memory_v2").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

        let memory_limits = &LinuxMemory {
            limit: None,
            swap: Some(512),
            reservation: None,
            kernel: None,
            kernel_tcp: None,
            swappiness: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    #[test]
    fn test_err_bad_limit_v2() {
        let tmp = create_temp_dir("test_err_bad_limit_v2").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

        let memory_limits = &LinuxMemory {
            limit: Some(-2),
            swap: None,
            reservation: None,
            kernel: None,
            kernel_tcp: None,
            swappiness: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    #[test]
    fn test_err_bad_swap_v2() {
        let tmp = create_temp_dir("test_err_bad_swap_v2").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

        let memory_limits = &LinuxMemory {
            limit: Some(512),
            swap: Some(-3),
            reservation: None,
            kernel: None,
            kernel_tcp: None,
            swappiness: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    quickcheck! {
        fn property_test_set_memory_v2(linux_memory: LinuxMemory) -> bool {
            let tmp = create_temp_dir("property_test_set_memory_v2").expect("create temp directory for test");
            set_fixture(&tmp, CGROUP_MEMORY_MAX, "0").expect("set fixture for memory limit");
            set_fixture(&tmp, CGROUP_MEMORY_LOW, "0").expect("set fixture for memory reservation");
            set_fixture(&tmp, CGROUP_MEMORY_SWAP, "0").expect("set fixture for swap limit");

            let result = Memory::apply(&tmp, &linux_memory);

            // we need to check for expected errors first and foremost or we'll get false negatives
            // later
            if let Some(limit) = linux_memory.limit {
                if limit < -1 {
                    return result.is_err();
                }
            }

            if let Some(swap) = linux_memory.swap {
                if swap < -1 {
                    return result.is_err();
                }
                if linux_memory.limit.is_none() {
                    return result.is_err();
                }
            }

            if let Some(reservation) = linux_memory.reservation {
                if reservation < -1 {
                    return result.is_err();
                }
            }

            // check the limit file is set as expected
            let limit_content = read_to_string(tmp.join(CGROUP_MEMORY_MAX)).expect("read memory limit to string");
            let limit_check = match linux_memory.limit {
                Some(limit) if limit == -1 => limit_content == "max",
                Some(limit) => limit_content == limit.to_string(),
                None => limit_content == "0",
            };

            // check the swap file is set as expected
            let swap_content = read_to_string(tmp.join(CGROUP_MEMORY_SWAP)).expect("read swap limit to string");
            let swap_check = match linux_memory.swap {
                Some(swap) if swap == -1 => swap_content == "max",
                Some(swap) => swap_content == swap.to_string(),
                None => {
                    match linux_memory.limit {
                        Some(limit) if limit == -1 => swap_content == "max",
                        _ => swap_content == "0",
                    }
                }
            };


            // check the resevation file is set as expected
            let reservation_content = read_to_string(tmp.join(CGROUP_MEMORY_LOW)).expect("read memory reservation to string");
            let reservation_check = match linux_memory.reservation {
                Some(reservation) if reservation == -1 => reservation_content == "max",
                Some(reservation) => reservation_content == reservation.to_string(),
                None => reservation_content == "0",
            };

            println!("limit_check: {}", limit_check);
            println!("swap_check: {}", swap_check);
            println!("reservation_check: {}", reservation_check);
            limit_check && swap_check && reservation_check
        }
    }
}
