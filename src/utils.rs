//! Utility functionality

use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{self, DirBuilder, File};
use std::ops::Deref;
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::prelude::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use anyhow::{bail, Result};
use nix::sys::stat::Mode;
use nix::sys::statfs;
use nix::unistd;

pub trait PathBufExt {
    fn as_in_container(&self) -> Result<PathBuf>;
    fn join_absolute_path(&self, p: &Path) -> Result<PathBuf>;
}

impl PathBufExt for PathBuf {
    fn as_in_container(&self) -> Result<PathBuf> {
        if self.is_relative() {
            bail!("Relative path cannot be converted to the path in the container.")
        } else {
            let path_string = self.to_string_lossy().into_owned();
            Ok(PathBuf::from(path_string[1..].to_string()))
        }
    }

    fn join_absolute_path(&self, p: &Path) -> Result<PathBuf> {
        if !p.is_absolute() && !p.as_os_str().is_empty() {
            bail!(
                "cannot join {:?} because it is not the absolute path.",
                p.display()
            )
        }
        Ok(PathBuf::from(format!("{}{}", self.display(), p.display())))
    }
}

pub fn parse_env(envs: &[String]) -> HashMap<String, String> {
    envs.iter()
        .filter_map(|e| {
            let mut split = e.split('=');

            if let Some(key) = split.next() {
                let value: String = split.collect::<Vec<&str>>().join("=");
                Some((String::from(key), value))
            } else {
                None
            }
        })
        .collect()
}

pub fn do_exec(path: impl AsRef<Path>, args: &[String]) -> Result<()> {
    let p = CString::new(path.as_ref().to_string_lossy().to_string())?;
    let a: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();
    unistd::execvp(&p, &a)?;
    Ok(())
}

// TODO implement
pub fn set_name(_name: &str) -> Result<()> {
    Ok(())
}

/// If None, it will generate a default path for cgroups.
pub fn get_cgroup_path(cgroups_path: &Option<PathBuf>, container_id: &str) -> PathBuf {
    match cgroups_path {
        Some(cpath) => cpath.clone(),
        None => PathBuf::from(container_id),
    }
}

pub fn delete_with_retry<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut attempts = 0;
    let mut delay = Duration::from_millis(10);
    let path = path.as_ref();

    while attempts < 5 {
        if fs::remove_dir(path).is_ok() {
            return Ok(());
        }

        std::thread::sleep(delay);
        attempts += attempts;
        delay *= attempts;
    }

    bail!("could not delete {:?}", path)
}

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();
    fs::write(path, contents).with_context(|| format!("failed to write to {:?}", path))?;
    Ok(())
}

pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path).with_context(|| format!("failed to create directory {:?}", path))
}

pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::open(path).with_context(|| format!("failed to open {:?}", path))
}

/// Creates the specified directory and all parent directories with the specified mode. Ensures
/// that the directory has been created with the correct mode and that the owner of the directory
/// is the owner that has been specified
/// # Example
/// ``` no_run
/// use youki::utils::create_dir_all_with_mode;
/// use nix::sys::stat::Mode;
/// use std::path::Path;
///
/// let path = Path::new("/tmp/youki");
/// create_dir_all_with_mode(&path, 1000, Mode::S_IRWXU).unwrap();
/// assert!(path.exists())
/// ```
pub fn create_dir_all_with_mode<P: AsRef<Path>>(path: P, owner: u32, mode: Mode) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        DirBuilder::new()
            .recursive(true)
            .mode(mode.bits())
            .create(path)
            .with_context(|| format!("failed to create directory {}", path.display()))?;
    }

    let metadata = path
        .metadata()
        .with_context(|| format!("failed to get metadata for {}", path.display()))?;

    if metadata.is_dir()
        && metadata.st_uid() == owner
        && metadata.st_mode() & mode.bits() == mode.bits()
    {
        Ok(())
    } else {
        bail!(
            "metadata for {} does not possess the expected attributes",
            path.display()
        );
    }
}

// Make sure a given path is on procfs. This is to avoid the security risk that
// /proc path is mounted over. Ref: CVE-2019-16884
pub fn ensure_procfs(path: &Path) -> Result<()> {
    let procfs_fd = fs::File::open(path)?;
    let fstat_info = statfs::fstatfs(&procfs_fd.as_raw_fd())?;

    if fstat_info.filesystem_type() != statfs::PROC_SUPER_MAGIC {
        bail!(format!("{:?} is not on the procfs", path));
    }

    Ok(())
}

pub struct TempDir {
    path: Option<PathBuf>,
}

impl TempDir {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let p = path.into();
        std::fs::create_dir_all(&p)
            .with_context(|| format!("failed to create directory {}", p.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_absolute_path() {
        assert_eq!(
            PathBuf::from("sample/a/")
                .join_absolute_path(&PathBuf::from("/b"))
                .unwrap(),
            PathBuf::from("sample/a/b")
        );
    }

    #[test]
    fn test_join_absolute_path_error() {
        assert!(PathBuf::from("sample/a/")
            .join_absolute_path(&PathBuf::from("b/c"))
            .is_err(),);
    }

    #[test]
    fn test_get_cgroup_path() {
        let cid = "sample_container_id";
        assert_eq!(
            get_cgroup_path(&None, cid),
            PathBuf::from("sample_container_id")
        );
        assert_eq!(
            get_cgroup_path(&Some(PathBuf::from("/youki")), cid),
            PathBuf::from("/youki")
        );
    }
    #[test]
    fn test_parse_env() -> Result<()> {
        let key = "key".to_string();
        let value = "value".to_string();
        let env_input = vec![format!("{}={}", key, value)];
        let env_output = parse_env(&env_input);
        assert_eq!(
            env_output.len(),
            1,
            "There should be exactly one entry inside"
        );
        assert_eq!(env_output.get_key_value(&key), Some((&key, &value)));

        Ok(())
    }
}
