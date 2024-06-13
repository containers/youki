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

// function compatible with setDisabled in go-selinux repo.
// set_disabled disables SELinux support for the package.
pub fn set_disabled() {
    unimplemented!("not implemented yet")
}

// function compatible with getEnabled in go-selinux repo.
// get_enabled returns whether SELinux is enabled or not.
pub fn get_enabled() -> bool {
    unimplemented!("not implemented yet")
}

// function compatible with classIndex in go-selinux repo.
// classIndex returns the int index for an object class in the loaded policy,
// or -1 and an error.
pub fn class_index(class: &str) -> Result<i64, String> {
    unimplemented!("not implemented yet")
}

// function compatible with setFileLabel in go-selinux repo.
// set_file_label sets the SELinux label for this path, following symlinks, or returns an error.
pub fn set_file_label(fpath: &str, label: &str) -> Result<(), std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }

    loop {
        match set_xattr(fpath, XATTR_NAME_SELINUX, label.as_bytes(), 0) {
            Ok(_) => break,
            // TODO: This line will be fixed after implementing set_xattr.
            Err(EINTR) => continue,
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

// function compatible with lSetFileLabel in go-selinux repo.
// lset_file_label sets the SELinux label for this path, not following symlinks,
// or returns an error.
pub fn lset_file_label(fpath: &str, label: &str) -> Result<(), std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }

    loop {
        match lset_xattr(fpath, XATTR_NAME_SELINUX, label.as_bytes(), 0) {
            Ok(_) => break,
            // TODO: This line will be fixed after implementing lset_xattr.
            Err(EINTR) => continue,
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

// function compatible with fileLabel in go-selinux repo.
// fileLabel returns the SELinux label for this path, following symlinks,
// or returns an error.
pub fn file_label(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    get_xattr(fpath, XATTR_NAME_SELINUX)
}

// function compatible with lFileLabel in go-selinux repo.
// lfile_label returns the SELinux label for this path, not following symlinks,
// or returns an error.
pub fn lfile_label(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    lget_xattr(fpath, XATTR_NAME_SELINUX)
}

// function compatible with setFSCreateLabel in go-selinux repo.
// set_fscreate_label sets the default label the kernel which the kernel is using
// for file system objects.
pub fn set_fscreate_label(label: &str) -> Result<(), std::io::Error> {
    return write_con(attr_path("fscreate").as_str(), label);
}

// function compatible with fsCreateLabel in go-selinux repo.
// fscreate_label returns the default label the kernel which the kernel is using
// for file system objects created by this task. "" indicates default.
pub fn fscreate_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("fscreate").as_str());
}

// function compatible with currentLabel in go-selinux repo.
// current_label returns the SELinux label of the current process thread, or an error.
pub fn current_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("current").as_str());
}

// function compatible with pidLabel in go-selinux repo.
// pid_label returns the SELinux label of the given pid, or an error.
pub fn pid_label(pid: i64) -> Result<String, std::io::Error> {
    let label = format!("/proc/{}/attr/current", pid);
    return read_con(label.as_str());
}

// function compatible with execLabel in go-selinux repo.
// exec_label returns the SELinux label that the kernel will use for any programs
// that are executed by the current process thread, or an error.
pub fn exec_label() -> Result<String, std::io::Error> {
    return read_con(attr_path("exec").as_str());
}

// function compatible with SetExecLabel in go-selinux repo.
// set_exec_label sets the SELinux label that the kernel will use for any programs
// that are executed by the current process thread, or an error.
pub fn set_exec_label(label: &str) {
    unimplemented!("not implemented yet")
}

// function compatible with SetTaskLabel in go-selinux repo.
// set_task_label sets the SELinux label for the current thread, or an error.
// This requires the dyntransition permission.
pub fn set_task_label(label: &str) {
    unimplemented!("not implemented yet")
}

// function compatible with SetSocketLabel in go-selinux repo.
// set_socket_label takes a process label and tells the kernel to assign the
// label to the next socket that gets created.
pub fn set_socket_label(label: &str) {
    unimplemented!("not implemented yet")
}

// function compatible with SocketLabel in go-selinux repo.
// socket_label retrieves the current socket label setting.
pub fn socket_label() {
    unimplemented!("not implemented yet")
}

// function compatible with peerLabel in go-selinux repo.
// peer_label retrieves the label of the client on the other side of a socket.
pub fn peer_label() {
    unimplemented!("not implemented yet")
}

// function compatible with setKeyLabel in go-selinux repo.
// set_key_label takes a process label and tells the kernel to assign the
// label to the next kernel keyring that gets created.
pub fn set_key_label(label: &str) -> Result<(), std::io::Error> {
    match write_con("/proc/self/attr/keycreate", label) {
        Ok(v) => Ok(v),
        // TODO: This line will be fixed after implementing write_con.
        Err(e) => Err(e),
    }
}

// function compatible with KeyLabel in go-selinux repo.
// key_label retrieves the current kernel keyring label setting
pub fn key_label() {
    unimplemented!("not implemented yet")
}

// function compatible with clearLabels in go-selinux repo.
// clear_labels clears all reserved labels. 
pub fn clear_labels() {
    unimplemented!("not implemented yet")
}

// function compatible with reserveLabel in go-selinux repo.
// reserve_label reserves the MLS/MCS level component of the specified label
pub fn reserve_label(label: &str) {
    unimplemented!("not implemented yet")
}

// function compatible with roFileLabel in go-selinux repo.
// ro_file_label returns the specified SELinux readonly file label
pub fn ro_file_label() {
    unimplemented!("not implemented yet")
}

// function compatible with kvmContainerLabels in go-selinux repo.
// kvm_container_labels returns the default processLabel and mountLabel to be used
// for kvm containers by the calling process.
pub fn kvm_container_labels() {
    unimplemented!("not implemented yet")
}

// function compatible with initContainerLabels in go-selinux repo.
// init_container_labels returns the default processLabel and file labels to be
// used for containers running an init system like systemd by the calling process.
pub fn init_container_labels() {
    unimplemented!("not implemented yet")
}

// function compatible with containerLabels in go-selinux repo.
// container_labels returns an allocated processLabel and fileLabel to be used for
// container labeling by the calling process.
pub fn container_labels() {
    unimplemented!("not implemented yet")
}

// function compatible with PrivContainerMountLabel in go-selinux repo.
// priv_container_mount_label returns mount label for privileged containers.
pub fn priv_container_mount_label() {
    unimplemented!("not implemented yet")
}

// function compatible with FormatMountLabel in go-selinux repo.
// format_mount_label returns a string to be used by the mount command.
// Using the SELinux `context` mount option.
// Changing labels of files on mount points with this option can never be changed.
// format_mount_label returns a string to be used by the mount command.
// The format of this string will be used to alter the labeling of the mountpoint.
// The string returned is suitable to be used as the options field of the mount command.
// If you need to have additional mount point options, you can pass them in as
// the first parameter. The second parameter is the label that you wish to apply
// to all content in the mount point.
pub fn format_mount_label(src: &str, mount_label: &str) -> String {
    return format_mount_label_by_type(src, mount_label, "context")
}

// function compatible with FormatMountLabelByType in go-selinux repo.
// format_mount_label_by_type returns a string to be used by the mount command.
// Allow caller to specify the mount options. For example using the SELinux
// `fscontext` mount option would allow certain container processes to change
// labels of files created on the mount points, where as `context` option does not.
pub fn format_mount_label_by_type(src: &str, mount_label: &str, context_type: &str) -> String {
    let mut formatted_src = src.to_owned();

    if !mount_label.is_empty() {
        if formatted_src.is_empty() {
            formatted_src = format!("{}=\"{}\"", context_type, mount_label);
        } else {
            formatted_src = format!("{},{}=\"{}\"", formatted_src, context_type, mount_label);
        }
    }
    formatted_src
}

// function compatible with writeCon in go-selinux repo.
pub fn write_con(fpath: &str, val: &str) -> Result<(), std::io::Error> {
    unimplemented!("not implemented yet");
}

// function compatible with isProcHandle in go-selinux repo.
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

// function compatible with readConFd in go-selinux repo.
pub fn read_con_fd(file: &mut File) -> Result<String, std::io::Error> {
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    
    // Remove null bytes on the end of a file.
    let trimmed_data = data.trim_end_matches(char::from(0));
    Ok(trimmed_data.to_string())
}

// function compatible with readCon in go-selinux repo.
pub fn read_con(fpath: &str) -> Result<String, std::io::Error> {
    if fpath.is_empty() {
        return Err(std::io::Error::new(io::ErrorKind::InvalidInput, ERR_EMPTY_PATH));
    }
    let mut in_file = File::open(fpath)?;

    is_proc_handle(&in_file)?;
    read_con_fd(&mut in_file)
}

// function compatible with attrPath in go-selinux repo.
pub fn attr_path(attr: &str) -> String {
    // Linux >= 3.17 provides this
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

    format!("/proc/self/task/{}/attr/{}", gettid(), attr)
}

#[cfg(test)]
mod tests {
    use crate::selinux::*;
    use std::fs::{self, File};
    use std::path::Path;
    use std::io::Write;
    use std::io::Seek;

    fn create_temp_file(content: &[u8], file_name: &str) {
        let path = Path::new(file_name);
        let mut file = File::create(&path).expect("Failed to create file");
        file.write_all(content).expect("Failed to write to file");
        file.sync_all().expect("Failed to sync file");
    }

    #[test]
    fn test_format_mount_label() {
        let src_list = vec!["", "src", "src"];
        let mount_label_list = vec!["foobar", "foobar", ""];
        let expected_list = vec!["context=\"foobar\"", "src,context=\"foobar\"", "src"];
        for (i, src) in src_list.iter().enumerate() {
            let mount_label = mount_label_list[i];
            let expected = expected_list[i];
            assert_eq!(
                format_mount_label(src, mount_label),
                expected
            );
        }
    }

    #[test]
    fn test_format_mount_label_by_type() {
        let src_list = vec!["", "src", "src"];
        let mount_label_list = vec!["foobar", "foobar", ""];
        let context_list = vec!["fscontext", "fscontext", "rootcontext"];
        let expected_list = vec!["fscontext=\"foobar\"", "src,fscontext=\"foobar\"", "src"];
        for (i, src) in src_list.iter().enumerate() {
            let mount_label = mount_label_list[i];
            let context = context_list[i];
            let expected = expected_list[i];
            assert_eq!(
                format_mount_label_by_type(src, mount_label, context),
                expected
            );
        }
    }    

    #[test]
    fn test_read_con_fd() {
        let content_list: Vec<&[u8]> = vec![b"Hello, world\0", b"Hello, world\0\0\0", b"Hello,\0world"];
        let expected_list = vec!["Hello, world", "Hello, world", "Hello,\0world"];
        let file_name = "test.txt";
        for (i, content) in content_list.iter().enumerate() {
            let expected = expected_list[i];
            create_temp_file(content, file_name);
            // Need to open again to get read permission.
            let mut file = File::open(file_name).expect("Failed to open file");
            let result = read_con_fd(&mut file).expect("Failed to read file");
            assert_eq!(result, expected);
            fs::remove_file(file_name).expect("Failed to remove test file");
        }
    }
}

