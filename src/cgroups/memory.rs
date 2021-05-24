use std::io::{prelude::*, Write};
use std::{
    fs::{create_dir_all, OpenOptions},
    path::Path,
};

use anyhow::{Result, *};
use nix::{errno::Errno, unistd::Pid};

use crate::{
    cgroups::Controller,
    spec::{LinuxMemory, LinuxResources},
};

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

impl Controller for Memory {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::info!(
            "Memory controller path: {}",
            cgroup_root.to_str().unwrap_or("")
        );
        create_dir_all(&cgroup_root)?;

        if let Some(memory) = &linux_resources.memory {
            let reservation = memory.reservation.unwrap_or(0);

            Self::apply(&memory, cgroup_root)?;

            if reservation != 0 {
                Self::set(reservation, &cgroup_root.join(CGROUP_MEMORY_RESERVATION))?;
            }

            if linux_resources.disable_oom_killer {
                Self::set(0, &cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL))?;
            } else {
                Self::set(1, &cgroup_root.join(CGROUP_MEMORY_OOM_CONTROL))?;
            }

            if let Some(swappiness) = memory.swappiness {
                if swappiness <= 100 {
                    Self::set(swappiness, &cgroup_root.join(CGROUP_MEMORY_SWAPPINESS))?;
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
                Self::set(kmem, &cgroup_root.join(CGROUP_KERNEL_MEMORY_LIMIT))?;
            }
            if let Some(tcp_mem) = memory.kernel_tcp {
                Self::set(tcp_mem, &cgroup_root.join(CGROUP_KERNEL_TCP_MEMORY_LIMIT))?;
            }

            OpenOptions::new()
                .create(false)
                .write(true)
                .truncate(false)
                .open(cgroup_root.join("cgroup.procs"))?
                .write_all(pid.to_string().as_bytes())?;
        }
        Ok(())
    }
}

impl Memory {
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
        let path = cgroup_root.join(CGROUP_MEMORY_LIMIT);

        match Self::set(val, &path) {
            Ok(_) => Ok(()),
            Err(e) => {
                // we need to look into the raw OS error for an EBUSY status
                if let Some(code) = e.raw_os_error() {
                    // the nix crate has a handy enum for these, lets use that
                    let errno = Errno::from_i32(code);
                    // if the error is EBUSY
                    if let Errno::EBUSY = errno {
                        let usage = Self::get_memory_usage(cgroup_root)?;
                        let max_usage = Self::get_memory_max_usage(cgroup_root)?;
                        Err(anyhow!(
                            "unable to set memory limit to {} (current usage: {}, peak usage: {})",
                            val,
                            usage,
                            max_usage,
                        ))
                    } else {
                        Err(anyhow!(e))
                    }
                } else {
                    Err(anyhow!(e))
                }
            }
        }
    }

    fn set_swap(val: i64, cgroup_root: &Path) -> Result<()> {
        if val == 0 {
            return Ok(());
        }

        let path = cgroup_root.join(CGROUP_MEMORY_SWAP_LIMIT);

        Self::set(val, &path)?;

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
        match resource.limit {
            Some(limit) => {
                let current_limit = Self::get_memory_limit(cgroup_root)?;
                match resource.swap {
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
            None => match resource.swap {
                Some(swap) => {
                    Self::set_memory_and_swap(0, swap, false, cgroup_root)?;
                }
                None => {
                    Self::set_memory_and_swap(0, 0, false, cgroup_root)?;
                }
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::LinuxMemory;

    fn set_fixture(temp_dir: &std::path::Path, filename: &str, val: &str) -> Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(temp_dir.join(filename))?
            .write_all(val.as_bytes())?;

        Ok(())
    }

    fn create_temp_dir(test_name: &str) -> Result<std::path::PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

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
            let linux_memory = &LinuxMemory {
                limit: Some(limit),
                swap: None, // Some(0) gives the same outcome
                reservation: None,
                kernel: None,
                kernel_tcp: None,
                swappiness: None,
            };
            Memory::apply(linux_memory, &tmp).expect("Set memory and swap");

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
            Memory::apply(linux_memory, &tmp).expect("Set memory and swap");

            let limit_content =
                std::fs::read_to_string(tmp.join(CGROUP_MEMORY_LIMIT)).expect("Read to string");
            assert_eq!(limit.to_string(), limit_content);

            let swap_content = std::fs::read_to_string(tmp.join(CGROUP_MEMORY_SWAP_LIMIT))
                .expect("Read to string");
            assert_eq!(swap.to_string(), swap_content);
        }
    }
}
