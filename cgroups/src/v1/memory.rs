use std::collections::HashMap;
use std::io::{prelude::*, Write};
use std::{fs::OpenOptions, path::Path};

use anyhow::{anyhow, bail, Result};
use nix::errno::Errno;

use super::Controller;
use crate::common::{self, ControllerOpt};
use crate::stats::{self, parse_single_value, MemoryData, MemoryStats, StatsProvider};

use oci_spec::runtime::LinuxMemory;

const CGROUP_MEMORY_SWAP_LIMIT: &str = "memory.memsw.limit_in_bytes";
const CGROUP_MEMORY_LIMIT: &str = "memory.limit_in_bytes";
const CGROUP_MEMORY_USAGE: &str = "memory.usage_in_bytes";
const CGROUP_MEMORY_MAX_USAGE: &str = "memory.max_usage_in_bytes";
const CGROUP_MEMORY_SWAPPINESS: &str = "memory.swappiness";
const CGROUP_MEMORY_RESERVATION: &str = "memory.soft_limit_in_bytes";
const CGROUP_MEMORY_OOM_CONTROL: &str = "memory.oom_control";

const CGROUP_KERNEL_MEMORY_LIMIT: &str = "memory.kmem.limit_in_bytes";
const CGROUP_KERNEL_TCP_MEMORY_LIMIT: &str = "memory.kmem.tcp.limit_in_bytes";

// Shows various memory statistics
const MEMORY_STAT: &str = "memory.stat";
//
const MEMORY_USE_HIERARCHY: &str = "memory.use_hierarchy";
// Prefix for memory cgroup files
const MEMORY_PREFIX: &str = "memory";
// Prefix for memory and swap cgroup files
const MEMORY_AND_SWAP_PREFIX: &str = "memory.memsw";
// Prefix for kernel memory cgroup files
const MEMORY_KERNEL_PREFIX: &str = "memory.kmem";
// Prefix for kernel tcp memory cgroup files
const MEMORY_KERNEL_TCP_PREFIX: &str = "memory.kmem.tcp";
// Memory usage in bytes
const MEMORY_USAGE_IN_BYTES: &str = ".usage_in_bytes";
// Maximum recorded memory usage
const MEMORY_MAX_USAGE_IN_BYTES: &str = ".max_usage_in_bytes";
// Memory usage limit in bytes
const MEMORY_LIMIT_IN_BYTES: &str = ".limit_in_bytes";
// Number of times memory usage hit limits
const MEMORY_FAIL_COUNT: &str = ".failcnt";

pub struct Memory {}

impl Controller for Memory {
    type Resource = LinuxMemory;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Memory cgroup config");

        if let Some(memory) = Self::needs_to_handle(controller_opt) {
            let reservation = memory.reservation().unwrap_or(0);

            Self::apply(memory, cgroup_root)?;

            if reservation != 0 {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_MEMORY_RESERVATION),
                    reservation,
                )?;
            }

            if controller_opt.disable_oom_killer {
                common::write_cgroup_file(cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL), 0)?;
            } else {
                common::write_cgroup_file(cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL), 1)?;
            }

            if let Some(swappiness) = memory.swappiness() {
                if swappiness <= 100 {
                    common::write_cgroup_file(
                        cgroup_root.join(CGROUP_MEMORY_SWAPPINESS),
                        swappiness,
                    )?;
                } else {
                    // invalid swappiness value
                    return Err(anyhow!(
                        "Invalid swappiness value: {}. Valid range is 0-100",
                        swappiness
                    ));
                }
            }

            // NOTE: Seems as though kernel and kernelTCP are both deprecated
            // neither are implemented by runc. Tests pass without this, but
            // kept in per the spec.
            if let Some(kmem) = memory.kernel() {
                common::write_cgroup_file(cgroup_root.join(CGROUP_KERNEL_MEMORY_LIMIT), kmem)?;
            }
            if let Some(tcp_mem) = memory.kernel_tcp() {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_KERNEL_TCP_MEMORY_LIMIT),
                    tcp_mem,
                )?;
            }
        }

        Ok(())
    }

    fn needs_to_handle<'a>(_controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        // TODO: fix compile error
        // error[E0515]: cannot return value referencing temporary value
        // if let Some(memory) = &controller_opt.resources.memory() {
        //     return Some(memory);
        // }
        None
    }
}

impl StatsProvider for Memory {
    type Stats = MemoryStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let memory = Self::get_memory_data(cgroup_path, MEMORY_PREFIX)?;
        let memswap = Self::get_memory_data(cgroup_path, MEMORY_AND_SWAP_PREFIX)?;
        let kernel = Self::get_memory_data(cgroup_path, MEMORY_KERNEL_PREFIX)?;
        let kernel_tcp = Self::get_memory_data(cgroup_path, MEMORY_KERNEL_TCP_PREFIX)?;
        let hierarchy = Self::hierarchy_enabled(cgroup_path)?;
        let stats = Self::get_stat_data(cgroup_path)?;

        Ok(MemoryStats {
            memory,
            memswap,
            kernel,
            kernel_tcp,
            cache: stats["cache"],
            hierarchy,
            stats,
        })
    }
}

impl Memory {
    fn get_memory_data(cgroup_path: &Path, file_prefix: &str) -> Result<MemoryData> {
        let memory_data = MemoryData {
            usage: parse_single_value(
                &cgroup_path.join(format!("{}{}", file_prefix, MEMORY_USAGE_IN_BYTES)),
            )?,
            max_usage: parse_single_value(
                &cgroup_path.join(format!("{}{}", file_prefix, MEMORY_MAX_USAGE_IN_BYTES)),
            )?,
            limit: parse_single_value(
                &cgroup_path.join(format!("{}{}", file_prefix, MEMORY_LIMIT_IN_BYTES)),
            )?,
            fail_count: parse_single_value(
                &cgroup_path.join(format!("{}{}", file_prefix, MEMORY_FAIL_COUNT)),
            )?,
        };

        Ok(memory_data)
    }

    fn hierarchy_enabled(cgroup_path: &Path) -> Result<bool> {
        let hierarchy_path = cgroup_path.join(MEMORY_USE_HIERARCHY);
        let hierarchy = common::read_cgroup_file(hierarchy_path)?;
        let enabled = matches!(hierarchy.trim(), "1");

        Ok(enabled)
    }

    fn get_stat_data(cgroup_path: &Path) -> Result<HashMap<String, u64>> {
        stats::parse_flat_keyed_data(&cgroup_path.join(MEMORY_STAT))
    }

    fn get_memory_usage(cgroup_root: &Path) -> Result<u64> {
        let path = cgroup_root.join(CGROUP_MEMORY_USAGE);
        let mut contents = String::new();
        OpenOptions::new()
            .create(false)
            .read(true)
            .open(path)?
            .read_to_string(&mut contents)?;

        contents = contents.trim().to_string();

        if contents == "max" {
            return Ok(u64::MAX);
        }

        let val = contents.parse::<u64>()?;
        Ok(val)
    }

    fn get_memory_max_usage(cgroup_root: &Path) -> Result<u64> {
        let path = cgroup_root.join(CGROUP_MEMORY_MAX_USAGE);
        let mut contents = String::new();
        OpenOptions::new()
            .create(false)
            .read(true)
            .open(path)?
            .read_to_string(&mut contents)?;

        contents = contents.trim().to_string();

        if contents == "max" {
            return Ok(u64::MAX);
        }

        let val = contents.parse::<u64>()?;
        Ok(val)
    }

    fn get_memory_limit(cgroup_root: &Path) -> Result<i64> {
        let path = cgroup_root.join(CGROUP_MEMORY_LIMIT);
        let mut contents = String::new();
        OpenOptions::new()
            .create(false)
            .read(true)
            .open(path)?
            .read_to_string(&mut contents)?;

        contents = contents.trim().to_string();

        if contents == "max" {
            return Ok(i64::MAX);
        }

        let val = contents.parse::<i64>()?;
        Ok(val)
    }

    fn set<T: ToString>(val: T, path: &Path) -> std::io::Result<()> {
        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(path)?
            .write_all(val.to_string().as_bytes())?;
        Ok(())
    }

    fn set_memory(val: i64, cgroup_root: &Path) -> Result<()> {
        if val == 0 {
            return Ok(());
        }
        let path = cgroup_root.join(CGROUP_MEMORY_LIMIT);

        match Self::set(val, &path) {
            Ok(_) => Ok(()),
            Err(e) => {
                // we need to look into the raw OS error for an EBUSY status
                match e.raw_os_error() {
                    Some(code) => match Errno::from_i32(code) {
                        Errno::EBUSY => {
                            let usage = Self::get_memory_usage(cgroup_root)?;
                            let max_usage = Self::get_memory_max_usage(cgroup_root)?;
                            bail!(
                                    "unable to set memory limit to {} (current usage: {}, peak usage: {})",
                                    val,
                                    usage,
                                    max_usage,
                            )
                        }
                        _ => bail!(e),
                    },
                    None => bail!(e),
                }
            }
        }
    }

    fn set_swap(swap: i64, cgroup_root: &Path) -> Result<()> {
        if swap == 0 {
            return Ok(());
        }

        common::write_cgroup_file(cgroup_root.join(CGROUP_MEMORY_SWAP_LIMIT), swap)?;
        Ok(())
    }

    fn set_memory_and_swap(
        limit: i64,
        swap: i64,
        is_updated: bool,
        cgroup_root: &Path,
    ) -> Result<()> {
        // According to runc we need to change the write sequence of
        // limit and swap so it won't fail, because the new and old
        // values don't fit the kernel's validation
        // see:
        // https://github.com/opencontainers/runc/blob/3f6594675675d4e88901c782462f56497260b1d2/libcontainer/cgroups/fs/memory.go#L89
        if is_updated {
            Self::set_swap(swap, cgroup_root)?;
            Self::set_memory(limit, cgroup_root)?;
        }
        Self::set_memory(limit, cgroup_root)?;
        Self::set_swap(swap, cgroup_root)?;
        Ok(())
    }

    fn apply(resource: &LinuxMemory, cgroup_root: &Path) -> Result<()> {
        match resource.limit() {
            Some(limit) => {
                let current_limit = Self::get_memory_limit(cgroup_root)?;
                match resource.swap() {
                    Some(swap) => {
                        let is_updated = swap == -1 || current_limit < swap;
                        Self::set_memory_and_swap(limit, swap, is_updated, cgroup_root)?;
                    }
                    None => {
                        if limit == -1 {
                            Self::set_memory_and_swap(limit, -1, true, cgroup_root)?;
                        } else {
                            let is_updated = current_limit < 0;
                            Self::set_memory_and_swap(limit, 0, is_updated, cgroup_root)?;
                        }
                    }
                }
            }
            None => match resource.swap() {
                Some(swap) => Self::set_memory_and_swap(0, swap, false, cgroup_root)?,
                None => Self::set_memory_and_swap(0, 0, false, cgroup_root)?,
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::CGROUP_PROCS;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::{LinuxMemoryBuilder, LinuxResourcesBuilder};

    #[test]
    fn test_set_memory() {
        let limit = 1024;
        let tmp = create_temp_dir("test_set_memory").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_USAGE, "0").expect("Set fixure for memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_MAX_USAGE, "0").expect("Set fixure for max memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_LIMIT, "0").expect("Set fixure for memory limit");
        Memory::set_memory(limit, &tmp).expect("Set memory limit");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
        assert_eq!(limit.to_string(), content)
    }

    #[test]
    fn pass_set_memory_if_limit_is_zero() {
        let sample_val = "1024";
        let limit = 0;
        let tmp = create_temp_dir("pass_set_memory_if_limit_is_zero")
            .expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_LIMIT, sample_val).expect("Set fixure for memory limit");
        Memory::set_memory(limit, &tmp).expect("Set memory limit");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
        assert_eq!(content, sample_val)
    }

    #[test]
    fn test_set_swap() {
        let limit = 512;
        let tmp = create_temp_dir("test_set_swap").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP_LIMIT, "0").expect("Set fixure for swap limit");
        Memory::set_swap(limit, &tmp).expect("Set swap limit");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAP_LIMIT)).expect("Read to string");
        assert_eq!(limit.to_string(), content)
    }

    #[test]
    fn test_set_memory_and_swap() {
        let tmp =
            create_temp_dir("test_set_memory_and_swap").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_USAGE, "0").expect("Set fixure for memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_MAX_USAGE, "0").expect("Set fixure for max memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_LIMIT, "0").expect("Set fixure for memory limit");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP_LIMIT, "0").expect("Set fixure for swap limit");

        // test unlimited memory with no set swap
        {
            let limit = -1;
            let linux_memory = LinuxMemoryBuilder::default().limit(limit).build().unwrap();
            Memory::apply(&linux_memory, &tmp).expect("Set memory and swap");

            let limit_content =
                std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
            assert_eq!(limit.to_string(), limit_content);

            let swap_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAP_LIMIT))
                .expect("Read to string");
            // swap should be set to -1 also
            assert_eq!(limit.to_string(), swap_content);
        }

        // test setting swap and memory to arbitrary values
        {
            let limit = 1024 * 1024 * 1024;
            let swap = 1024;
            let linux_memory = LinuxMemoryBuilder::default()
                .limit(limit)
                .swap(swap)
                .build()
                .unwrap();
            Memory::apply(&linux_memory, &tmp).expect("Set memory and swap");

            let limit_content =
                std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
            assert_eq!(limit.to_string(), limit_content);

            let swap_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAP_LIMIT))
                .expect("Read to string");
            assert_eq!(swap.to_string(), swap_content);
        }
    }

    quickcheck! {
            fn property_test_set_memory(linux_memory: LinuxMemory, disable_oom_killer: bool) -> bool {
                let tmp =
                    create_temp_dir("property_test_set_memory").expect("create temp directory for test");
                set_fixture(&tmp, CGROUP_MEMORY_USAGE, "0").expect("Set fixure for memory usage");
                set_fixture(&tmp, CGROUP_MEMORY_MAX_USAGE, "0").expect("Set fixure for max memory usage");
                set_fixture(&tmp, CGROUP_MEMORY_LIMIT, "0").expect("Set fixure for memory limit");
                set_fixture(&tmp, CGROUP_MEMORY_SWAP_LIMIT, "0").expect("Set fixure for swap limit");
                set_fixture(&tmp, CGROUP_MEMORY_SWAPPINESS, "0").expect("Set fixure for swappiness");
                set_fixture(&tmp, CGROUP_MEMORY_RESERVATION, "0").expect("Set fixture for memory reservation");
                set_fixture(&tmp, CGROUP_MEMORY_OOM_CONTROL, "0").expect("Set fixture for oom control");
                set_fixture(&tmp, CGROUP_KERNEL_MEMORY_LIMIT, "0").expect("Set fixture for kernel memory limit");
                set_fixture(&tmp, CGROUP_KERNEL_TCP_MEMORY_LIMIT, "0").expect("Set fixture for kernel tcp memory limit");
                set_fixture(&tmp, CGROUP_PROCS, "").expect("set fixture for proc file");


                // clone to avoid use of moved value later on
                let memory_limits = linux_memory;

                let linux_resources = LinuxResourcesBuilder::default().devices(vec![]).memory(linux_memory).hugepage_limits(vec![]).build().unwrap();

    <<<<<<< HEAD
                let controller_opt = ControllerOpt {
                    resources: linux_resources,
                    disable_oom_killer,
                    ..Default::default()
                };
    =======
            let controller_opt = ControllerOpt {
                resources: &linux_resources,
                disable_oom_killer,
                oom_score_adj: None,
                freezer_state: None,
            };
    >>>>>>> upstream/main

                let result = <Memory as Controller>::apply(&controller_opt, &tmp);


                if result.is_err() {
                    if let Some(swappiness) = memory_limits.swappiness() {
                        // error is expected if swappiness is greater than 100
                        if swappiness > 100 {
                            return true;
                        }
                    } else {
                        // useful for debugging
                        println!("Some unexpected error: {:?}", result.unwrap_err());
                        // any other error should be considered unexpected
                        return false;
                    }
                }

                // check memory reservation
                let reservation_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_RESERVATION)).expect("read memory reservation");
                let reservation_check = match memory_limits.reservation() {
                    Some(reservation) => {
                        reservation_content == reservation.to_string()
                    }
                    None => reservation_content == "0",
                };

                // check kernel memory limit
                let kernel_content = std::fs::read_to_string(tmp.join(CGROUP_KERNEL_MEMORY_LIMIT)).expect("read kernel memory limit");
                let kernel_check = match memory_limits.kernel() {
                    Some(kernel) => {
                        kernel_content == kernel.to_string()
                    }
                    None => kernel_content == "0",
                };

                // check kernel tcp memory limit
                let kernel_tcp_content = std::fs::read_to_string(tmp.join(CGROUP_KERNEL_TCP_MEMORY_LIMIT)).expect("read kernel tcp memory limit");
                let kernel_tcp_check = match memory_limits.kernel_tcp() {
                    Some(kernel_tcp) => {
                        kernel_tcp_content == kernel_tcp.to_string()
                    }
                    None => kernel_tcp_content == "0",
                };

                // check swappiness
                let swappiness_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAPPINESS)).expect("read swappiness");
                let swappiness_check = match memory_limits.swappiness() {
                    Some(swappiness) if swappiness <= 100 => {
                        swappiness_content == swappiness.to_string()
                    }
                    None => swappiness_content == "0",
                    // everything else is a failure
                    _ => false,
                };

                // check limit and swap
                let limit_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("read memory limit");
                let swap_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAP_LIMIT)).expect("read swap memory limit");
                let limit_swap_check = match memory_limits.limit() {
                    Some(limit) => {
                        match memory_limits.swap() {
                            Some(swap) => {
                                limit_content == limit.to_string()
                                    && swap_content == swap.to_string()
                            }
                            None => {
                                if limit == -1 {
                                    limit_content == limit.to_string()
                                        && swap_content == "-1"
                                } else {
                                    limit_content == limit.to_string()
                                        && swap_content == "0"
                                }
                            }
                        }
                    }
                    None => {
                        match memory_limits.swap() {
                            Some(swap) => {
                                limit_content == "0"
                                    && swap_content == swap.to_string()
                            }
                            None => limit_content == "0" && swap_content == "0"
                        }
                    }
                };

                // useful for debugging
                println!("reservation_check: {:?}", reservation_check);
                println!("kernel_check: {:?}", kernel_check);
                println!("kernel_tcp_check: {:?}", kernel_tcp_check);
                println!("swappiness_check: {:?}", swappiness_check);
                println!("limit_swap_check: {:?}", limit_swap_check);

                // combine all the checks
                reservation_check && kernel_check && kernel_tcp_check && swappiness_check && limit_swap_check
            }
        }

    #[test]
    fn test_stat_memory_data() {
        let tmp = create_temp_dir("test_stat_memory_data").expect("create test directory");
        set_fixture(
            &tmp,
            &format!("{}{}", MEMORY_PREFIX, MEMORY_USAGE_IN_BYTES),
            "1024\n",
        )
        .unwrap();
        set_fixture(
            &tmp,
            &format!("{}{}", MEMORY_PREFIX, MEMORY_MAX_USAGE_IN_BYTES),
            "2048\n",
        )
        .unwrap();
        set_fixture(
            &tmp,
            &format!("{}{}", MEMORY_PREFIX, MEMORY_LIMIT_IN_BYTES),
            "4096\n",
        )
        .unwrap();
        set_fixture(
            &tmp,
            &format!("{}{}", MEMORY_PREFIX, MEMORY_FAIL_COUNT),
            "5\n",
        )
        .unwrap();

        let actual = Memory::get_memory_data(&tmp, MEMORY_PREFIX).expect("get cgroup stats");
        let expected = MemoryData {
            usage: 1024,
            max_usage: 2048,
            limit: 4096,
            fail_count: 5,
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_stat_hierarchy_enabled() {
        let tmp = create_temp_dir("test_stat_hierarchy_enabled").expect("create test directory");
        set_fixture(&tmp, MEMORY_USE_HIERARCHY, "1").unwrap();

        let enabled = Memory::hierarchy_enabled(&tmp).expect("get cgroup stats");
        assert!(enabled)
    }

    #[test]
    fn test_stat_hierarchy_disabled() {
        let tmp = create_temp_dir("test_stat_hierarchy_disabled").expect("create test directory");
        set_fixture(&tmp, MEMORY_USE_HIERARCHY, "0").unwrap();

        let enabled = Memory::hierarchy_enabled(&tmp).expect("get cgroup stats");
        assert!(!enabled)
    }

    #[test]
    fn test_stat_memory_stats() {
        let tmp = create_temp_dir("test_stat_memory_stats").expect("create test directory");
        let content = [
            "cache 0",
            "rss 0",
            "rss_huge 0",
            "shmem 0",
            "pgpgout 0",
            "unevictable 0",
            "hierarchical_memory_limit 9223372036854771712",
            "hierarchical_memsw_limit 9223372036854771712",
        ]
        .join("\n");
        set_fixture(&tmp, MEMORY_STAT, &content).unwrap();

        let actual = Memory::get_stat_data(&tmp).expect("get cgroup data");
        let expected: HashMap<String, u64> = [
            ("cache".to_owned(), 0),
            ("rss".to_owned(), 0),
            ("rss_huge".to_owned(), 0),
            ("shmem".to_owned(), 0),
            ("pgpgout".to_owned(), 0),
            ("unevictable".to_owned(), 0),
            ("hierarchical_memory_limit".to_owned(), 9223372036854771712),
            ("hierarchical_memsw_limit".to_owned(), 9223372036854771712),
        ]
        .iter()
        .cloned()
        .collect();

        assert_eq!(actual, expected);
    }
}
