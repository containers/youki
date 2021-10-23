//! Utility functionality

use anyhow::Context;
use anyhow::{bail, Result};
use nix::sys::stat::Mode;
use nix::sys::statfs;
use nix::unistd;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{self, DirBuilder, File};
use std::io::ErrorKind;
use std::ops::Deref;
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::prelude::{AsRawFd, OsStrExt};
use std::path::{Path, PathBuf};

pub trait PathBufExt {
    fn as_relative(&self) -> Result<&Path>;
    fn join_safely<P: AsRef<Path>>(&self, p: P) -> Result<PathBuf>;
}

impl PathBufExt for Path {
    fn as_relative(&self) -> Result<&Path> {
        if self.is_relative() {
            bail!("relative path cannot be converted to the path in the container.")
        } else {
            self.strip_prefix("/")
                .with_context(|| format!("failed to strip prefix from {:?}", self))
        }
    }

    fn join_safely<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let path = path.as_ref();
        if path.is_relative() {
            return Ok(self.join(path));
        }

        let stripped = path
            .strip_prefix("/")
            .with_context(|| format!("failed to strip prefix from {}", path.display()))?;
        Ok(self.join(stripped))
    }
}

pub fn parse_env(envs: &[String]) -> HashMap<String, String> {
    envs.iter()
        .filter_map(|e| {
            let mut split = e.split('=');

            split.next().map(|key| {
                let value = split.collect::<Vec<&str>>().join("=");
                (key.into(), value)
            })
        })
        .collect()
}

pub fn do_exec(path: impl AsRef<Path>, args: &[String]) -> Result<()> {
    let p = CString::new(path.as_ref().as_os_str().as_bytes())
        .with_context(|| format!("failed to convert path {:?} to cstring", path.as_ref()))?;
    let a: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.as_bytes()).unwrap_or_default())
        .collect();
    unistd::execvp(&p, &a)?;
    Ok(())
}

/// If None, it will generate a default path for cgroups.
pub fn get_cgroup_path(cgroups_path: &Option<PathBuf>, container_id: &str) -> PathBuf {
    match cgroups_path {
        Some(cpath) => cpath.clone(),
        None => PathBuf::from(container_id),
    }
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
/// use libcontainer::utils::create_dir_all_with_mode;
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

pub fn secure_join<P: Into<PathBuf>>(rootfs: P, unsafe_path: P) -> Result<PathBuf> {
    let mut rootfs = rootfs.into();
    let mut path = unsafe_path.into();
    let mut clean_path = PathBuf::new();

    let mut part = path.iter();
    let mut i = 0;

    loop {
        if i > 255 {
            bail!("dereference too many symlinks, may be infinite loop");
        }

        let part_path;
        match part.next() {
            Some(part) => {
                part_path = PathBuf::from(part);
            }
            None => {
                break;
            }
        }

        if !part_path.is_absolute() {
            if part_path.starts_with("..") {
                clean_path.pop();
            } else {
                // check if symlink then dereference
                let curr_path = PathBuf::from(&rootfs).join(&clean_path).join(&part_path);
                let metadata = match curr_path.symlink_metadata() {
                    Ok(metadata) => Some(metadata),
                    Err(error) => match error.kind() {
                        // if file does not exists, treat it as normal path
                        ErrorKind::NotFound => None,
                        other_error => {
                            bail!(
                                "unable to obtain symlink metadata for file {:?}: {:?}",
                                curr_path,
                                other_error
                            );
                        }
                    },
                };

                if let Some(metadata) = metadata {
                    if metadata.file_type().is_symlink() {
                        let link_path = fs::read_link(curr_path)?;
                        path = link_path.join(part.as_path());
                        part = path.iter();

                        // increase after dereference symlink
                        i += 1;
                        continue;
                    }
                }

                clean_path.push(&part_path);
            }
        }
    }

    rootfs.push(clean_path);
    Ok(rootfs)
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
pub(crate) mod test_utils {
    use crate::process::channel;
    use anyhow::Context;
    use anyhow::{bail, Result};
    use nix::sys::wait;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestResult {
        success: bool,
        message: String,
    }

    pub fn test_in_child_process<F: FnOnce() -> Result<()>>(cb: F) -> Result<()> {
        let (mut sender, mut receiver) = channel::channel::<TestResult>()?;
        match unsafe { nix::unistd::fork()? } {
            nix::unistd::ForkResult::Parent { child } => {
                let res = receiver.recv()?;
                wait::waitpid(child, None)?;

                if !res.success {
                    bail!("child process failed: {}", res.message);
                }
            }
            nix::unistd::ForkResult::Child => {
                let test_result = match cb() {
                    Ok(_) => TestResult {
                        success: true,
                        message: String::new(),
                    },
                    Err(err) => TestResult {
                        success: false,
                        message: err.to_string(),
                    },
                };
                sender
                    .send(test_result)
                    .context("failed to send from the child process")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[test]
    fn test_secure_join() {
        assert_eq!(
            secure_join(Path::new("/tmp/rootfs"), Path::new("path")).unwrap(),
            PathBuf::from("/tmp/rootfs/path")
        );
        assert_eq!(
            secure_join(Path::new("/tmp/rootfs"), Path::new("more/path")).unwrap(),
            PathBuf::from("/tmp/rootfs/more/path")
        );
        assert_eq!(
            secure_join(Path::new("/tmp/rootfs"), Path::new("/absolute/path")).unwrap(),
            PathBuf::from("/tmp/rootfs/absolute/path")
        );
        assert_eq!(
            secure_join(
                Path::new("/tmp/rootfs"),
                Path::new("/path/with/../parent/./sample")
            )
            .unwrap(),
            PathBuf::from("/tmp/rootfs/path/parent/sample")
        );
        assert_eq!(
            secure_join(Path::new("/tmp/rootfs"), Path::new("/../../../../tmp")).unwrap(),
            PathBuf::from("/tmp/rootfs/tmp")
        );
        assert_eq!(
            secure_join(Path::new("/tmp/rootfs"), Path::new("./../../../../var/log")).unwrap(),
            PathBuf::from("/tmp/rootfs/var/log")
        );
        assert_eq!(
            secure_join(
                Path::new("/tmp/rootfs"),
                Path::new("../../../../etc/passwd")
            )
            .unwrap(),
            PathBuf::from("/tmp/rootfs/etc/passwd")
        );
    }
    #[test]
    fn test_secure_join_symlink() {
        use std::os::unix::fs::symlink;

        let tmp = create_temp_dir("root").unwrap();
        let test_root_dir = tmp.path();

        symlink("somepath", PathBuf::from(&test_root_dir).join("etc")).unwrap();
        symlink(
            "../../../../../../../../../../../../../etc",
            PathBuf::from(&test_root_dir).join("longbacklink"),
        )
        .unwrap();
        symlink(
            "/../../../../../../../../../../../../../etc/passwd",
            PathBuf::from(&test_root_dir).join("absolutelink"),
        )
        .unwrap();

        assert_eq!(
            secure_join(test_root_dir, PathBuf::from("etc").as_path()).unwrap(),
            PathBuf::from(&test_root_dir).join("somepath")
        );
        assert_eq!(
            secure_join(test_root_dir, PathBuf::from("longbacklink").as_path()).unwrap(),
            PathBuf::from(&test_root_dir).join("somepath")
        );
        assert_eq!(
            secure_join(test_root_dir, PathBuf::from("absolutelink").as_path()).unwrap(),
            PathBuf::from(&test_root_dir).join("somepath/passwd")
        );
    }
}
