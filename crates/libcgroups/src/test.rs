#![cfg(test)]

use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
};

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

pub fn create_temp_dir(test_name: &str) -> Result<TempDir> {
    let dir = TempDir::new(std::env::temp_dir().join(test_name))?;
    Ok(dir)
}

pub fn setup(testname: &str, cgroup_file: &str) -> (TempDir, PathBuf) {
    let tmp = create_temp_dir(testname).expect("create temp directory for test");
    let cgroup_file = set_fixture(&tmp, cgroup_file, "")
        .unwrap_or_else(|_| panic!("set test fixture for {cgroup_file}"));

    (tmp, cgroup_file)
}

pub fn set_fixture(temp_dir: &Path, filename: &str, val: &str) -> Result<PathBuf> {
    let full_path = temp_dir.join(filename);

    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&full_path)
        .with_context(|| format!("failed to open {full_path:?}"))?
        .write_all(val.as_bytes())
        .with_context(|| format!("failed to write to {full_path:?}"))?;

    Ok(full_path)
}
