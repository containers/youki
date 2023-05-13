//! Utility functionality

use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{self, DirBuilder, File};
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::prelude::{AsRawFd, OsStrExt};
use std::path::{Component, Path, PathBuf};

use nix::sys::stat::Mode;
use nix::sys::statfs;
use nix::unistd;
use nix::unistd::{Uid, User};

#[derive(Debug, thiserror::Error)]
pub enum PathBufExtError {
    #[error("relative path cannot be converted to the path in the container")]
    RelativePath,
    #[error("failed to strip prefix from {path:?}")]
    StripPrefix {
        path: PathBuf,
        source: std::path::StripPrefixError,
    },
    #[error("failed to canonicalize path {path:?}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to get current directory")]
    CurrentDir { source: std::io::Error },
}

pub trait PathBufExt {
    fn as_relative(&self) -> Result<&Path, PathBufExtError>;
    fn join_safely<P: AsRef<Path>>(&self, p: P) -> Result<PathBuf, PathBufExtError>;
    fn canonicalize_safely(&self) -> Result<PathBuf, PathBufExtError>;
    fn normalize(&self) -> PathBuf;
}

impl PathBufExt for Path {
    fn as_relative(&self) -> Result<&Path, PathBufExtError> {
        match self.is_relative() {
            true => Err(PathBufExtError::RelativePath),
            false => Ok(self
                .strip_prefix("/")
                .map_err(|e| PathBufExtError::StripPrefix {
                    path: self.to_path_buf(),
                    source: e,
                })?),
        }
    }

    fn join_safely<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, PathBufExtError> {
        let path = path.as_ref();
        if path.is_relative() {
            return Ok(self.join(path));
        }

        let stripped = path
            .strip_prefix("/")
            .map_err(|e| PathBufExtError::StripPrefix {
                path: self.to_path_buf(),
                source: e,
            })?;
        Ok(self.join(stripped))
    }

    /// Canonicalizes existing and not existing paths
    fn canonicalize_safely(&self) -> Result<PathBuf, PathBufExtError> {
        if self.exists() {
            self.canonicalize()
                .map_err(|e| PathBufExtError::Canonicalize {
                    path: self.to_path_buf(),
                    source: e,
                })
        } else {
            if self.is_relative() {
                let p = std::env::current_dir()
                    .map_err(|e| PathBufExtError::CurrentDir { source: e })?
                    .join(self);
                return Ok(p.normalize());
            }

            Ok(self.normalize())
        }
    }

    /// Normalizes a path. In contrast to canonicalize the path does not need to exist.
    // adapted from https://github.com/rust-lang/cargo/blob/fede83ccf973457de319ba6fa0e36ead454d2e20/src/cargo/util/paths.rs#L61
    fn normalize(&self) -> PathBuf {
        let mut components = self.components().peekable();
        let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
            components.next();
            PathBuf::from(c.as_os_str())
        } else {
            PathBuf::new()
        };

        for component in components {
            match component {
                Component::Prefix(..) => unreachable!(),
                Component::RootDir => {
                    ret.push(component.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    ret.pop();
                }
                Component::Normal(c) => {
                    ret.push(c);
                }
            }
        }
        ret
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

/// Get a nix::unistd::User via UID. Potential errors will be ignored.
pub fn get_unix_user(uid: Uid) -> Option<User> {
    match User::from_uid(uid) {
        Ok(x) => x,
        Err(_) => None,
    }
}

/// Get home path of a User via UID.
pub fn get_user_home(uid: u32) -> Option<PathBuf> {
    match get_unix_user(Uid::from_raw(uid)) {
        Some(user) => Some(user.dir),
        None => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DoExecError {
    #[error("failed to convert path to cstring")]
    PathToCString {
        source: std::ffi::NulError,
        path: PathBuf,
    },
    #[error("failed to execvp")]
    Execvp { source: nix::Error },
}

pub fn do_exec(path: impl AsRef<Path>, args: &[String]) -> Result<(), DoExecError> {
    let p = CString::new(path.as_ref().as_os_str().as_bytes()).map_err(|e| {
        DoExecError::PathToCString {
            source: e,
            path: path.as_ref().to_path_buf(),
        }
    })?;
    let c_args: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.as_bytes()).unwrap_or_default())
        .collect();
    unistd::execvp(&p, &c_args).map_err(|err| DoExecError::Execvp { source: err })?;

    Ok(())
}

/// If None, it will generate a default path for cgroups.
pub fn get_cgroup_path(
    cgroups_path: &Option<PathBuf>,
    container_id: &str,
    rootless: bool,
) -> PathBuf {
    match cgroups_path {
        Some(cpath) => cpath.clone(),
        None => match rootless {
            false => PathBuf::from(container_id),
            true => PathBuf::from(format!(":youki:{container_id}")),
        },
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WrappedIOError {
    #[error("failed to read from {path:?}")]
    ReadFile {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to write to {path:?}")]
    WriteFile {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to open {path:?}")]
    Open {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to create directory {path:?}")]
    CreateDirAll {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to get metadata")]
    GetMetadata { source: std::io::Error },
    #[error("metada doesn't match the expected attributes")]
    MetadataMismatch,
}

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> Result<(), WrappedIOError> {
    fs::write(path.as_ref(), contents).map_err(|err| WrappedIOError::WriteFile {
        source: err,
        path: path.as_ref().to_path_buf(),
    })
}

pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<(), WrappedIOError> {
    fs::create_dir_all(path.as_ref()).map_err(|err| WrappedIOError::CreateDirAll {
        source: err,
        path: path.as_ref().to_path_buf(),
    })
}

pub fn open<P: AsRef<Path>>(path: P) -> Result<File, WrappedIOError> {
    File::open(path.as_ref()).map_err(|err| WrappedIOError::Open {
        source: err,
        path: path.as_ref().to_path_buf(),
    })
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
pub fn create_dir_all_with_mode<P: AsRef<Path>>(
    path: P,
    owner: u32,
    mode: Mode,
) -> Result<(), WrappedIOError> {
    let path = path.as_ref();
    if !path.exists() {
        DirBuilder::new()
            .recursive(true)
            .mode(mode.bits())
            .create(path)
            .map_err(|err| WrappedIOError::CreateDirAll {
                source: err,
                path: path.to_path_buf(),
            })?;
    }

    let metadata = path
        .metadata()
        .map_err(|err| WrappedIOError::GetMetadata { source: err })?;

    if metadata.is_dir()
        && metadata.st_uid() == owner
        && metadata.st_mode() & mode.bits() == mode.bits()
    {
        Ok(())
    } else {
        Err(WrappedIOError::MetadataMismatch)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EnsureProcfsError {
    #[error("failed to open {path:?}")]
    OpenProcfs {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to get statfs for {path:?}")]
    StatfsProcfs { source: nix::Error, path: PathBuf },
    #[error("{path:?} is not on the procfs")]
    NotProcfs { path: PathBuf },
}

// Make sure a given path is on procfs. This is to avoid the security risk that
// /proc path is mounted over. Ref: CVE-2019-16884
pub fn ensure_procfs(path: &Path) -> Result<(), EnsureProcfsError> {
    let procfs_fd = fs::File::open(path).map_err(|err| EnsureProcfsError::OpenProcfs {
        source: err,
        path: path.to_path_buf(),
    })?;
    let fstat_info =
        statfs::fstatfs(&procfs_fd.as_raw_fd()).map_err(|err| EnsureProcfsError::StatfsProcfs {
            source: err,
            path: path.to_path_buf(),
        })?;

    if fstat_info.filesystem_type() != statfs::PROC_SUPER_MAGIC {
        return Err(EnsureProcfsError::NotProcfs {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

pub fn get_executable_path(name: &str, path_var: &str) -> Option<PathBuf> {
    let paths = path_var.trim_start_matches("PATH=");
    // if path has / in it, we have to assume absolute path, as per runc impl
    if name.contains('/') && PathBuf::from(name).exists() {
        return Some(PathBuf::from(name));
    }
    for path in paths.split(':') {
        let potential_path = PathBuf::from(path).join(name);
        if potential_path.exists() {
            return Some(potential_path);
        }
    }
    None
}

pub fn is_executable(path: &Path) -> Result<bool, std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = path.metadata()?;
    let permissions = metadata.permissions();
    // we have to check if the path is file and the execute bit
    // is set. In case of directories, the execute bit is also set,
    // so have to check if this is a file or not
    Ok(metadata.is_file() && permissions.mode() & 0o001 != 0)
}

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::process::channel;
    use anyhow::Context;
    use anyhow::{bail, Result};
    use nix::sys::wait;
    use rand::Rng;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestResult {
        success: bool,
        message: String,
    }

    #[allow(dead_code)]
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

    pub fn gen_u32() -> u32 {
        rand::thread_rng().gen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    pub fn test_get_unix_user() {
        let user = get_unix_user(Uid::from_raw(0));
        assert_eq!(user.unwrap().name, "root");

        // for a non-exist UID
        let user = get_unix_user(Uid::from_raw(1000000000));
        assert!(user.is_none());
    }

    #[test]
    pub fn test_get_user_home() {
        let dir = get_user_home(0);
        assert_eq!(dir.unwrap().to_str().unwrap(), "/root");

        // for a non-exist UID
        let dir = get_user_home(1000000000);
        assert!(dir.is_none());
    }

    #[test]
    fn test_get_cgroup_path() {
        let cid = "sample_container_id";
        assert_eq!(
            get_cgroup_path(&None, cid, false),
            PathBuf::from("sample_container_id")
        );
        assert_eq!(
            get_cgroup_path(&Some(PathBuf::from("/youki")), cid, false),
            PathBuf::from("/youki")
        );
    }

    #[test]
    fn test_parse_env() -> Result<()> {
        let key = "key".to_string();
        let value = "value".to_string();
        let env_input = vec![format!("{key}={value}")];
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
    fn test_get_executable_path() {
        let non_existing_abs_path = "/some/non/existent/absolute/path";
        let existing_abs_path = "/usr/bin/sh";
        let existing_binary = "sh";
        let non_existing_binary = "non-existent";
        let path_value = "PATH=/usr/bin:/bin";

        assert_eq!(
            get_executable_path(existing_abs_path, path_value),
            Some(PathBuf::from(existing_abs_path))
        );
        assert_eq!(get_executable_path(non_existing_abs_path, path_value), None);

        assert_eq!(
            get_executable_path(existing_binary, path_value),
            Some(PathBuf::from("/usr/bin/sh"))
        );

        assert_eq!(get_executable_path(non_existing_binary, path_value), None);
    }

    #[test]
    fn test_is_executable() {
        let tmp = tempfile::tempdir().expect("create temp directory for test");
        let executable_path = PathBuf::from("/bin/sh");
        let directory_path = tmp.path();
        let non_executable_path = directory_path.join("non_executable_file");
        let non_existent_path = PathBuf::from("/some/non/existent/path");

        File::create(non_executable_path.as_path()).unwrap();

        assert!(is_executable(&non_existent_path).is_err());
        assert!(is_executable(&executable_path).unwrap());
        assert!(!is_executable(&non_executable_path).unwrap());
        assert!(!is_executable(directory_path).unwrap());
    }
}
