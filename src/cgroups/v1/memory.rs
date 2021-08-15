use std::{fs::{File,OpenOptions}, path::Path, str};

use anyhow::{Result, *};
use async_trait::async_trait;
use nix::errno::Errno;
use rio::{Rio, Ordering};

use crate::cgroups::common::{self};
use crate::cgroups::v1::Controller;
use oci_spec::{LinuxMemory, LinuxResources};

const CGROUP_MEMORY_SWAP_LIMIT: &str = "memory.memsw.limit_in_bytes";
const CGROUP_MEMORY_LIMIT: &str = "memory.limit_in_bytes";
const CGROUP_MEMORY_USAGE: &str = "memory.usage_in_bytes";
const CGROUP_MEMORY_MAX_USAGE: &str = "memory.max_usage_in_bytes";
const CGROUP_MEMORY_SWAPPINESS: &str = "memory.swappiness";
const CGROUP_MEMORY_RESERVATION: &str = "memory.soft_limit_in_bytes";
const CGROUP_MEMORY_OOM_CONTROL: &str = "memory.oom_control";

const CGROUP_KERNEL_MEMORY_LIMIT: &str = "memory.kmem.limit_in_bytes";
const CGROUP_KERNEL_TCP_MEMORY_LIMIT: &str = "memory.kmem.tcp.limit_in_bytes";

pub struct Memory {}

#[async_trait]
impl Controller for Memory {
    type Resource = LinuxMemory;

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Memory cgroup config");

        if let Some(memory) = Self::needs_to_handle(linux_resources) {
            let reservation = memory.reservation.unwrap_or(0);

            Self::apply(ring, &memory, cgroup_root).await?;

            if reservation != 0 {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_MEMORY_RESERVATION),
                    reservation,
                )?;
            }

            if linux_resources.disable_oom_killer {
                common::write_cgroup_file(cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL), 0)?;
            } else {
                common::write_cgroup_file(cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL), 1)?;
            }

            if let Some(swappiness) = memory.swappiness {
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
            if let Some(kmem) = memory.kernel {
                common::write_cgroup_file(cgroup_root.join(CGROUP_KERNEL_MEMORY_LIMIT), kmem)?;
            }
            if let Some(tcp_mem) = memory.kernel_tcp {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_KERNEL_TCP_MEMORY_LIMIT),
                    tcp_mem,
                )?;
            }
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(memory) = &linux_resources.memory {
            return Some(memory);
        }

        None
    }
}

impl Memory {
    async fn get_memory_usage(ring: &Rio, cgroup_root: &Path) -> Result<u64> {
        let path = cgroup_root.join(CGROUP_MEMORY_USAGE);
        let contents = common::async_read_cgroup_file(ring, path).await?;

        if contents == "max" {
            return Ok(u64::MAX);
        }

        let val = contents.parse::<u64>()?;
        Ok(val)
    }

    async fn get_memory_max_usage(ring: &Rio, cgroup_root: &Path) -> Result<u64> {
        let path = cgroup_root.join(CGROUP_MEMORY_MAX_USAGE);
        let contents = common::async_read_cgroup_file(ring, path).await?;

        if contents == "max" {
            return Ok(u64::MAX);
        }

        let val = contents.parse::<u64>()?;
        Ok(val)
    }

    async fn get_memory_limit(ring: &Rio, file: &File) -> Result<i64> {
        let mut buffer: Vec<u8> = Vec::new();

        ring.read_at_ordered(
            file,
            &mut buffer,
            0,
            Ordering::Link,
        ).await?;

        let contents = str::from_utf8(&buffer)?;

        if contents == "max" {
            return Ok(i64::MAX);
        }

        let val = contents.parse::<i64>()?;
        Ok(val)
    }

    async fn set<T: ToString>(ring: &Rio, val: T, file: &File) -> std::io::Result<usize> {
        ring.write_at_ordered(
            file,
            &val.to_string(),
            0,
            Ordering::Link,
        ).await
    }

    async fn set_memory(ring: &Rio, val: i64, memory_limit_file: &File, cgroup_root: &Path) -> Result<()> {
        if val == 0 {
            return Ok(());
        }

        match Self::set(ring, val, memory_limit_file).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // we need to look into the raw OS error for an EBUSY status
                match e.raw_os_error() {
                    Some(code) => match Errno::from_i32(code) {
                        Errno::EBUSY => {
                            let usage = Self::get_memory_usage(ring, cgroup_root).await?;
                            let max_usage = Self::get_memory_max_usage(ring, cgroup_root).await?;
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

    async fn set_swap(ring: &Rio, swap: i64, swap_limit_file: &File) -> Result<()> {
        if swap == 0 {
            return Ok(());
        }

        common::async_write_cgroup_file(ring, swap_limit_file, swap).await?;
        Ok(())
    }

    async fn set_memory_and_swap(
        ring: &Rio,
        limit: i64,
        swap: i64,
        is_updated: bool,
        memory_limit_file: &File,
        swap_limit_file: &File,
        cgroup_root: &Path,
    ) -> Result<()> {
        // According to runc we need to change the write sequence of
        // limit and swap so it won't fail, because the new and old
        // values don't fit the kernel's validation
        // see:
        // https://github.com/opencontainers/runc/blob/3f6594675675d4e88901c782462f56497260b1d2/libcontainer/cgroups/fs/memory.go#L89
        if is_updated {
            Self::set_swap(ring, swap, swap_limit_file).await?;
            Self::set_memory(ring, limit, memory_limit_file, cgroup_root).await?;
        }
        Self::set_memory(ring, limit, memory_limit_file, cgroup_root).await?;
        Self::set_swap(ring, swap, swap_limit_file).await?;
        Ok(())
    }

    async fn apply(ring: &Rio, resource: &LinuxMemory, cgroup_root: &Path) -> Result<()> {
        let path = cgroup_root.join(CGROUP_MEMORY_LIMIT);
        let memory_limit_file = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(path)?;
        let swap_limit_file = common::open_cgroup_file(cgroup_root.join(CGROUP_MEMORY_SWAP_LIMIT))?;
        match resource.limit {
            Some(limit) => {
                let current_limit = Self::get_memory_limit(ring, &memory_limit_file).await?;
                match resource.swap {
                    Some(swap) => {
                        let is_updated = swap == -1 || current_limit < swap;
                        Self::set_memory_and_swap(ring, limit, swap, is_updated, &memory_limit_file, &swap_limit_file, cgroup_root).await?;
                    }
                    None => {
                        if limit == -1 {
                            Self::set_memory_and_swap(ring, limit, -1, true, &memory_limit_file, &swap_limit_file, cgroup_root).await?;
                        } else {
                            let is_updated = current_limit < 0;
                            Self::set_memory_and_swap(ring, limit, 0, is_updated, &memory_limit_file, &swap_limit_file, cgroup_root).await?;
                        }
                    }
                }
            }
            None => match resource.swap {
                Some(swap) => Self::set_memory_and_swap(ring, 0, swap, false, &memory_limit_file, &swap_limit_file, cgroup_root).await?,
                None => Self::set_memory_and_swap(ring, 0, 0, false, &memory_limit_file, &swap_limit_file, cgroup_root).await?,
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::common::CGROUP_PROCS;
    use crate::cgroups::test::{set_fixture, aw};
    use crate::utils::create_temp_dir;
    use oci_spec::LinuxMemory;

    #[test]
    fn test_set_memory() {
        let limit = 1024;
        let tmp = create_temp_dir("test_set_memory").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_USAGE, "0").expect("Set fixure for memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_MAX_USAGE, "0").expect("Set fixure for max memory usage");
        set_fixture(&tmp, CGROUP_MEMORY_LIMIT, "0").expect("Set fixure for memory limit");
        let ring = rio::new().expect("start io_uring");
        let memory_limit_file = common::open_cgroup_file(tmp.join(CGROUP_MEMORY_LIMIT)).expect("open memory limit file");
        aw!(Memory::set_memory(&ring, limit, &memory_limit_file, &tmp)).expect("Set memory limit");
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
        let ring = rio::new().expect("start io_uring");
        let memory_limit_file = common::open_cgroup_file(tmp.join(CGROUP_MEMORY_LIMIT)).expect("open memory limit file");
        aw!(Memory::set_memory(&ring, limit, &memory_limit_file, &tmp)).expect("Set memory limit");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
        assert_eq!(content, sample_val)
    }

    #[test]
    fn test_set_swap() {
        let limit = 512;
        let tmp = create_temp_dir("test_set_swap").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_MEMORY_SWAP_LIMIT, "0").expect("Set fixure for swap limit");
        let ring = rio::new().expect("start io_uring");
        let swap_limit_file = common::open_cgroup_file(tmp.join(CGROUP_MEMORY_SWAP_LIMIT)).expect("open swap limit");

        aw!(Memory::set_swap(&ring, limit, &swap_limit_file)).expect("Set swap limit");
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
        let ring = rio::new().expect("start io_uring");

        // test unlimited memory with no set swap
        {
            let limit = -1;
            let linux_memory = &LinuxMemory {
                limit: Some(limit),
                swap: None, // Some(0) gives the same outcome
                reservation: None,
                kernel: None,
                kernel_tcp: None,
                swappiness: None,
            };
            aw!(Memory::apply(&ring, linux_memory, &tmp)).expect("Set memory and swap");

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
            let linux_memory = &LinuxMemory {
                limit: Some(limit),
                swap: Some(swap),
                reservation: None,
                kernel: None,
                kernel_tcp: None,
                swappiness: None,
            };
            aw!(Memory::apply(&ring, linux_memory, &tmp)).expect("Set memory and swap");

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
            let ring = rio::new().expect("start io_uring");


            // clone to avoid use of moved value later on
            let memory_limits = linux_memory.clone();

            let linux_resources = LinuxResources {
                devices: vec![],
                disable_oom_killer,
                oom_score_adj: None, // current unused
                memory: Some(linux_memory),
                cpu: None,
                pids: None,
                block_io: None,
                hugepage_limits: vec![],
                network: None,
                freezer: None,
            };

            let result = aw!(<Memory as Controller>::apply(&ring, &linux_resources, &tmp));

            if result.is_err() {
                if let Some(swappiness) = memory_limits.swappiness {
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
            let reservation_check = match memory_limits.reservation {
                Some(reservation) => {
                    reservation_content == reservation.to_string()
                }
                None => reservation_content == "0",
            };

            // check kernel memory limit
            let kernel_content = std::fs::read_to_string(tmp.join(CGROUP_KERNEL_MEMORY_LIMIT)).expect("read kernel memory limit");
            let kernel_check = match memory_limits.kernel {
                Some(kernel) => {
                    kernel_content == kernel.to_string()
                }
                None => kernel_content == "0",
            };

            // check kernel tcp memory limit
            let kernel_tcp_content = std::fs::read_to_string(tmp.join(CGROUP_KERNEL_TCP_MEMORY_LIMIT)).expect("read kernel tcp memory limit");
            let kernel_tcp_check = match memory_limits.kernel_tcp {
                Some(kernel_tcp) => {
                    kernel_tcp_content == kernel_tcp.to_string()
                }
                None => kernel_tcp_content == "0",
            };

            // check swappiness
            let swappiness_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAPPINESS)).expect("read swappiness");
            let swappiness_check = match memory_limits.swappiness {
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
            let limit_swap_check = match memory_limits.limit {
                Some(limit) => {
                    match memory_limits.swap {
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
                    match memory_limits.swap {
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
}
