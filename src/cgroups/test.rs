#![cfg(test)]

use anyhow::Result;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use oci_spec::LinuxCpu;

pub fn set_fixture(temp_dir: &Path, filename: &str, val: &str) -> Result<PathBuf> {
    let full_path = temp_dir.join(filename);

    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&full_path)?
        .write_all(val.as_bytes())?;

    Ok(full_path)
}

pub fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
    Ok(std::env::temp_dir().join(test_name))
}

pub struct LinuxCpuBuilder {
    resource: LinuxCpu,
}

impl LinuxCpuBuilder {
    pub fn new() -> Self {
        Self {
            resource: LinuxCpu {
                shares: None,
                quota: None,
                period: None,
                realtime_runtime: None,
                realtime_period: None,
                cpus: None,
                mems: None,
            },
        }
    }

    pub fn with_shares(mut self, shares: u64) -> Self {
        self.resource.shares = Some(shares);
        self
    }

    pub fn with_quota(mut self, quota: i64) -> Self {
        self.resource.quota = Some(quota);
        self
    }

    pub fn with_period(mut self, period: u64) -> Self {
        self.resource.period = Some(period);
        self
    }

    pub fn with_realtime_runtime(mut self, runtime: i64) -> Self {
        self.resource.realtime_runtime = Some(runtime);
        self
    }

    pub fn with_realtime_period(mut self, period: u64) -> Self {
        self.resource.realtime_period = Some(period);
        self
    }

    pub fn with_cpus(mut self, cpus: String) -> Self {
        self.resource.cpus = Some(cpus);
        self
    }

    pub fn with_mems(mut self, mems: String) -> Self {
        self.resource.mems = Some(mems);
        self
    }

    pub fn build(self) -> LinuxCpu {
        self.resource
    }
}
