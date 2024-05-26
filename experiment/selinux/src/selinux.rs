use std::sync::Once;
use crate::xattr::*;
use nix::unistd::gettid;
use nix::sys::statfs;
use nix::errno::Errno;
use std::path::PathBuf;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd};

const XATTR_NAME_SELINUX: &str = "security.selinux";
const ERR_EMPTY_PATH: &str = "empty path";
static ATTR_PATH_INIT: Once = Once::new();
static mut HAVE_THREAD_SELF: bool = false;

pub fn set_disabled() {
    panic!("not implemented yet")
}

pub fn get_enabled() -> bool {
    panic!("not implemented yet")
}

pub fn class_index(class: &str) -> Result<i64, String> {
    panic!("not implemented yet")
}

// set_file_label sets the SELinux label for this path, following symlinks, or returns an error.
pub fn set_file_label(fpath: &str, label: &str) -> Result<(), std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }

    loop {
        match set_xattr(fpath, XATTR_NAME_SELINUX, label.as_bytes(), 0) {
            Ok(_) => break,
// TODO            Err(Errno::EINTR) => continue,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("set_xattr failed: {}", e),
                ));
            }
        }
    }
    Ok(())
}

pub fn lset_file_label(fpath: &str, label: &str) -> Result<(), std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }

    loop {
        match lset_xattr(fpath, XATTR_NAME_SELINUX, label.as_bytes(), 0) {
            Ok(_) => break,
            // TODO: EINTR (after fixing lset_attr)
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("lset_xattr failed: {}", e),
                ));
            }
        }
    }
    Ok(())
}

pub fn file_label(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    let label = get_xattr(fpath, XATTR_NAME_SELINUX);
    match label {
        Ok(mut v) => {            
            if (v.len() > 0) && (v.chars().nth(v.len() - 1) == Some('\x00')) {
                v = (&v[0..v.len() - 1]).to_string();
            }
            return Ok(v);
        },
        Err(e) => return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("get_xattr failed: {}", e),
        ))
    }
}

pub fn lfile_label(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    let label = lget_xattr(fpath, XATTR_NAME_SELINUX);
    match label {
        Ok(mut v) => {            
            if (v.len() > 0) && (v.chars().nth(v.len() - 1) == Some('\x00')) {
                v = (&v[0..v.len() - 1]).to_string();
            }
            return Ok(v);
        },
        Err(e) => return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("lget_xattr failed: {}", e),
        ))
    }
}

pub fn set_fscreate_label(label: &str) -> Result<(), std::io::Error> {
    return write_con(attr_path("fscreate").as_str(), label);
}

pub fn fscreate_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("fscreate").as_str());
}

pub fn current_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("current").as_str());
}

pub fn pid_label(pid: i64) -> Result<String, std::io::Error> {
    let label = format!("/proc/{}/attr/current", pid);
    return read_con(label.as_str());
}

pub fn exec_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("exec").as_str());
}

pub fn set_exec_label(label: &str) {
    panic!("not implemented yet")
}

pub fn set_task_label(label: &str) {
    panic!("not implemented yet")
}

pub fn set_socket_label(label: &str) {
    panic!("not implemented yet")
}

pub fn socket_label() {
    panic!("not implemented yet")
}

pub fn peer_label() {
    panic!("not implemented yet")
}

pub fn set_key_label(label: &str) -> Result<(), std::io::Error> {
    match write_con("/proc/self/attr/keycreate", label) {
        Ok(v) => return Ok(v),
        //TODO: update error
        Err(e) => return Err(e),
    }
}

pub fn key_label() {
    panic!("not implemented yet")
}

pub fn clear_labels() {
    panic!("not implemented yet")
}

pub fn reserve_label(label: &str) {
    panic!("not implemented yet")
}

pub fn ro_file_label() {
    panic!("not implemented yet")
}

pub fn kvm_container_labels() {
    panic!("not implemented yet")
}

pub fn init_container_labels() {
    panic!("not implemented yet")
}

pub fn container_labels() {
    panic!("not implemented yet")
}

pub fn priv_container_mount_label() {
    panic!("not implemented yet")
}

pub fn format_mount_label(src: &str, mount_label: &str) -> String {
    return format_mount_label_by_type(src, mount_label, "context")
}

pub fn format_mount_label_by_type(src: &str, mount_label: &str, context_type: &str) -> String {
    let mut formatted_src = src.to_owned();

    if !mount_label.is_empty() {
        if formatted_src.is_empty() {
            formatted_src = format!("{}=\"{}\"", context_type, mount_label);
        } else {
            formatted_src = format!("{},{}=\"{}\"", formatted_src, context_type, mount_label);
        }
    }
    return formatted_src
}

pub fn write_con(fpath: &str, val: &str) -> Result<(), std::io::Error> {
    panic!("not implemented yet");
}

pub fn is_proc_handle(file: &File) -> Result<(), std::io::Error> {
    loop {
        match statfs::fstatfs(file.as_fd()) {
            Ok(stat) if stat.filesystem_type() == statfs::PROC_SUPER_MAGIC => break,
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other, format!("file {} is not on procfs", file.as_raw_fd())
                ));
            },
            Err(Errno::EINTR) => continue,
            Err(err) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("fstatfs failed: {}", err)))
            }
        }
    }
    Ok(())
}

pub fn read_con_fd(file: &mut File) -> Result<String, std::io::Error> {
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    let trimmed_data = data.trim_end_matches(char::from(0));
    Ok(trimmed_data.to_string())
}

pub fn read_con(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    let mut in_file = File::open(fpath)?;

    is_proc_handle(&in_file)?;
    return read_con_fd(&mut in_file);
}

pub fn attr_path(attr: &str) -> String {
    const THREAD_SELF_PREFIX: &str = "/proc/thread-self/attr";
    ATTR_PATH_INIT.call_once(|| {
        let path = PathBuf::from(THREAD_SELF_PREFIX);
        unsafe {
            HAVE_THREAD_SELF = path.is_dir();
        }
    });

    unsafe {
        if HAVE_THREAD_SELF {
            return format!("{}/{}", THREAD_SELF_PREFIX, attr);
        }
    }

    return format!("/proc/self/task/{}/attr/{}", gettid(), attr);
}

#[cfg(test)]
mod tests {
    use crate::selinux::*;

    #[test]
    fn test_format_mount_label() {
        assert_eq!(
            format_mount_label("", "foobar"),
            "context=\"foobar\""
        );

        assert_eq!(
            format_mount_label("src", "foobar"),
            "src,context=\"foobar\""
        );

        assert_eq!(
            format_mount_label("src", ""),
            "src"
        );

        assert_eq!(
            format_mount_label_by_type("", "foobar", "fscontext"),
            "fscontext=\"foobar\""
        );

        assert_eq!(
            format_mount_label_by_type("src", "foobar", "fscontext"),
            "src,fscontext=\"foobar\""
        );

        assert_eq!(
            format_mount_label_by_type("src", "", "rootcontext"),
            "src"
        );                                      
    }
}

