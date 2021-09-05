use anyhow::{bail, Context, Result};
use std::path::Path;

use oci_spec::runtime::{LinuxMemory};

use crate::{
    common::{self, ControllerOpt},
    stats::{self, MemoryData, MemoryStats, StatsProvider},
};

use super::controller::Controller;

const CGROUP_MEMORY_SWAP: &str = "memory.swap.max";
const CGROUP_MEMORY_MAX: &str = "memory.max";
const CGROUP_MEMORY_LOW: &str = "memory.low";
const MEMORY_STAT: &str = "memory.stat";

pub struct Memory {}

impl Controller for Memory {
    fn apply(controller_opt: &ControllerOpt, cgroup_path: &Path) -> Result<()> {
        if let Some(memory) = &controller_opt.resources.memory {
            Self::apply(cgroup_path, memory)
                .context("failed to apply memory resource restrictions")?;
        }

        Ok(())
    }
}

impl StatsProvider for Memory {
    type Stats = MemoryStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let stats = MemoryStats {
            memory: Self::get_memory_data(cgroup_path, "memory", "oom")?,
            memswap: Self::get_memory_data(cgroup_path, "memory.swap", "fail")?,
            hierarchy: true,
            stats: stats::parse_flat_keyed_data(&cgroup_path.join(MEMORY_STAT))?,
            ..Default::default()
        };

        Ok(stats)
    }
}

impl Memory {
    fn get_memory_data(
        cgroup_path: &Path,
        file_prefix: &str,
        fail_event: &str,
    ) -> Result<MemoryData> {
        let usage =
            stats::parse_single_value(&cgroup_path.join(format!("{}.{}", file_prefix, "current")))?;
        let limit =
            stats::parse_single_value(&cgroup_path.join(format!("{}.{}", file_prefix, "max")))?;

        let events = stats::parse_flat_keyed_data(
            &cgroup_path.join(format!("{}.{}", file_prefix, "events")),
        )?;
        let fail_count = if let Some((_, v)) = events.get_key_value(fail_event) {
            *v
        } else {
            Default::default()
        };

        Ok(MemoryData {
            usage,
            fail_count,
            limit,
            ..Default::default()
        })
    }

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
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::LinuxMemory;
    use std::fs::read_to_string;

    #[test]
    fn test_set_memory() {
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
            disable_oom_killer: None,
            use_hierarchy: None,
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
    fn test_set_memory_unlimited() {
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
            disable_oom_killer: None,
            use_hierarchy: None,
        };
        Memory::apply(&tmp, memory_limits).expect("apply memory limits");

        let limit_content = read_to_string(tmp.join(CGROUP_MEMORY_MAX)).expect("read memory limit");
        assert_eq!(limit_content, "max");

        let swap_content = read_to_string(tmp.join(CGROUP_MEMORY_SWAP)).expect("read swap limit");
        assert_eq!(swap_content, "max");
    }

    #[test]
    fn test_err_swap_no_memory() {
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
            disable_oom_killer: None,
            use_hierarchy: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    #[test]
    fn test_err_bad_limit() {
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
            disable_oom_killer: None,
            use_hierarchy: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    #[test]
    fn test_err_bad_swap() {
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
            disable_oom_killer: None,
            use_hierarchy: None,
        };

        let result = Memory::apply(&tmp, memory_limits);

        assert!(result.is_err());
    }

    quickcheck! {
        fn property_test_set_memory(linux_memory: LinuxMemory) -> bool {
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

    #[test]
    fn test_get_memory_data() {
        let tmp = create_temp_dir("test_stat_memory").expect("create test directory");
        set_fixture(&tmp, "memory.current", "12500\n").unwrap();
        set_fixture(&tmp, "memory.max", "25000\n").unwrap();
        let events = ["slab 5", "anon 13", "oom 3"].join("\n");
        set_fixture(&tmp, "memory.events", &events).unwrap();

        let actual = Memory::get_memory_data(&tmp, "memory", "oom").expect("get cgroup stats");
        let expected = MemoryData {
            usage: 12500,
            limit: 25000,
            fail_count: 3,
            ..Default::default()
        };

        assert_eq!(actual, expected);
    }
}
