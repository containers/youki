#![cfg(test)]

use anyhow::{Context, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use oci_spec::LinuxCpu;

use crate::utils::{create_temp_dir, TempDir};

pub fn setup(testname: &str, cgroup_file: &str) -> (TempDir, PathBuf) {
    let tmp = create_temp_dir(testname).expect("create temp directory for test");
    let cgroup_file = set_fixture(&tmp, cgroup_file, "")
        .unwrap_or_else(|_| panic!("set test fixture for {}", cgroup_file));

    (tmp, cgroup_file)
}

pub fn set_fixture(temp_dir: &Path, filename: &str, val: &str) -> Result<PathBuf> {
    let full_path = temp_dir.join(filename);

    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&full_path)
        .with_context(|| format!("failed to open {:?}", full_path))?
        .write_all(val.as_bytes())
        .with_context(|| format!("failed to write to {:?}", full_path))?;

    Ok(full_path)
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
