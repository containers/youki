use std::ffi::CString;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use nix::{env::clearenv, errno::Errno, unistd};

pub trait PathBufExt {
    fn as_in_container(&self) -> Result<PathBuf>;
    fn join_absolute_path(&self, p: &Path) -> Result<PathBuf>;
}

impl PathBufExt for PathBuf {
    fn as_in_container(&self) -> Result<PathBuf> {
        if self.is_relative() {
            bail!("Relative path cannnot be converted to the path in the container.")
        } else {
            let path_string = self.to_string_lossy().into_owned();
            Ok(PathBuf::from(path_string[1..].to_string()))
        }
    }

    fn join_absolute_path(&self, p: &Path) -> Result<PathBuf> {
        if !p.is_absolute() && !p.as_os_str().is_empty() {
            bail!(
                "connnot join {:?} because it is not the absolute path.",
                p.display()
            )
        }
        Ok(PathBuf::from(format!("{}{}", self.display(), p.display())))
    }
}

pub fn do_exec(path: impl AsRef<Path>, args: &[String], envs: &[String]) -> Result<()> {
    let p = CString::new(path.as_ref().to_string_lossy().to_string())?;
    let a: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();
    let envs: Vec<CString> = envs
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();

    unsafe {
        clearenv()?;
    }
    for e in envs {
        putenv(&e)?
    }

    unistd::execvp(&p, &a)?;
    Ok(())
}

#[inline]
fn putenv(string: &CString) -> nix::Result<()> {
    let ptr = string.clone().into_raw();
    let res = unsafe { libc::putenv(ptr as *mut libc::c_char) };
    Errno::result(res).map(drop)
}

// TODO implement
pub fn set_name(_name: &str) -> Result<()> {
    Ok(())
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
        assert_eq!(
            PathBuf::from("sample/a/")
                .join_absolute_path(&PathBuf::from("b/c"))
                .is_err(),
            true
        );
    }
}
