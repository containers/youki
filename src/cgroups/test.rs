use std::{io::Write, path::{Path, PathBuf}};
use anyhow::Result;

fn set_fixture(temp_dir: &Path, filename: &str, val: &str) -> Result<()> {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(temp_dir.join(filename))?
        .write_all(val.as_bytes())?;

    Ok(())
}

fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
    Ok(std::env::temp_dir().join(test_name))
}