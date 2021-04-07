use std::ffi::CString;
use std::path::PathBuf;

use anyhow::{bail, Result};
use nix::unistd;

pub trait PathBufExt {
    fn as_in_container(&self) -> Result<PathBuf>;
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
}

pub fn do_exec(path: &str, args: &[String]) -> Result<()> {
    let p = CString::new(path.to_string()).unwrap();
    let a: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();

    unistd::execvp(&p, &a)?;
    Ok(())
}

// TODO implement
pub fn set_name(_name: &str) -> Result<()> {
    // prctl::set_name(name).expect("set name failed.");
    // unsafe {
    //     let init = std::ffi::CString::new(name).expect("invalid process name");
    //     // let len = std::ffi::CStr::from_ptr(*ARGV).to_bytes().len();
    //     let len = std::ffi::CStr::from_ptr(0 as *mut i8).to_bytes().len();
    //     // after fork, ARGV points to the thread's local
    //     // copy of arg0.
    //     // libc::strncpy(*ARGV, init.as_ptr(), len);
    //     libc::strncpy(0 as *mut i8, init.as_ptr(), len);
    //     // no need to set the final character to 0 since
    //     // the initial string was already null-terminated.
    // }
    Ok(())
}
