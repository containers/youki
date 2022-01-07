use std::path::PathBuf;

use nix::sys::stat::stat;
use nix::sys::stat::SFlag;

pub enum AccessibilityStatus {
    Accessible,
    Blocked,
}

fn test_file_read_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    match std::fs::OpenOptions::new()
        .create(false)
        .read(true)
        .open(path)
    {
        Ok(_) => {
            // we can directly return accessible, as if we are allowed to open with read access,
            // we can read the file
            Ok(AccessibilityStatus::Accessible)
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                // we can get permission denied error if we try to read a
                // file which we do not have read read access for, in
                // which case that is not an error, but a valid accessibility status
                Ok(AccessibilityStatus::Blocked)
            } else {
                Err(e)
            }
        }
    }
}

fn test_dir_read_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    match std::fs::read_dir(path) {
        Ok(_) => Ok(AccessibilityStatus::Accessible),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Ok(AccessibilityStatus::Blocked)
            } else {
                Err(e)
            }
        }
    }
}

fn is_file_like(mode: u32) -> bool {
    // for this please refer
    // https://stackoverflow.com/questions/40163270/what-is-s-isreg-and-what-does-it-do
    // https://linux.die.net/man/2/stat
    mode & SFlag::S_IFREG.bits() != 0
        || mode & SFlag::S_IFBLK.bits() != 0
        || mode & SFlag::S_IFCHR.bits() != 0
}

fn is_dir(mode: u32) -> bool {
    mode & SFlag::S_IFDIR.bits() != 0
}

pub fn test_read_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    let fstat = stat(path)?;
    let mode = fstat.st_mode;
    if is_file_like(mode) {
        // we have a file or a char/block device
        return test_file_read_access(path);
    } else if is_dir(mode) {
        return test_dir_read_access(path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!(
            "cannot test read access for {:?}, has mode {:x}",
            path, mode
        ),
    ))
}

fn test_file_write_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    match std::fs::OpenOptions::new().write(true).open(path) {
        std::io::Result::Ok(_) => {
            // we don't have to check if we can actually write or not, as
            // if we are allowed to open file with write access, we can write to it
            Ok(AccessibilityStatus::Accessible)
        }
        std::io::Result::Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Ok(AccessibilityStatus::Blocked);
            } else {
                return Err(e);
            }
        }
    }
}

fn test_dir_write_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(PathBuf::from(path).join("test.txt"))
    {
        std::io::Result::Ok(_) => Ok(AccessibilityStatus::Accessible),
        std::io::Result::Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                // technically we can still get permission denied even if
                // we have write access, but do not have execute access, but by default
                // dirs are created with execute access so that should not be an issue
                return Ok(AccessibilityStatus::Blocked);
            } else {
                return Err(e);
            }
        }
    }
}

pub fn test_write_access(path: &str) -> Result<AccessibilityStatus, std::io::Error> {
    let fstat = stat(path)?;
    let mode = fstat.st_mode;
    if is_file_like(mode) {
        // we have a file or a char/block device
        return test_file_write_access(path);
    } else if is_dir(mode) {
        return test_dir_write_access(path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!(
            "cannot test write access for {:?}, has mode {:x}",
            path, mode
        ),
    ))
}
