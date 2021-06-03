#![cfg(test)]

use anyhow::Result;
use std::{
    fs,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
};

use oci_spec::LinuxCpu;

pub struct TempDir {
    path: Option<PathBuf>,
}

impl TempDir {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let p = path.into();
        std::fs::create_dir_all(&p)?;
        Ok(Self { path: Some(p) })
    }

    pub fn path(&self) -> &Path {
        self.path
            .as_ref()
            .expect("temp dir has already been removed")
    }

    pub fn remove(&mut self) {
        if let Some(p) = &self.path {
            let _ = fs::remove_dir_all(p);
            self.path = None;
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        self.remove();
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

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
        .open(&full_path)?
        .write_all(val.as_bytes())?;

    Ok(full_path)
}

pub fn create_temp_dir(test_name: &str) -> Result<TempDir> {
    let dir = TempDir::new(std::env::temp_dir().join(test_name))?;
    Ok(dir)
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
