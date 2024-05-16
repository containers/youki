#![cfg(test)]

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn setup(cgroup_file: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("create temp directory for test");
    let cgroup_file = set_fixture(tmp.path(), cgroup_file, "")
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
