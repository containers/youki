use crate::tools::*;
use nix::errno::Errno;
use nix::sys::socket::getsockopt;
use nix::sys::{statfs, statvfs};
use nix::unistd::gettid;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, AsRawFd};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const XATTR_NAME_SELINUX: &str = "security.selinux";
const ERR_EMPTY_PATH: &str = "empty path";
const KEY_LABEL_PATH: &str = "/proc/self/attr/keycreate";
const SELINUX_FS_MOUNT: &str = "/sys/fs/selinux";
const CONTEXT_FILE: &str = "/usr/share/containers/selinux/contexts";
const SELINUX_TYPE_TAG: &str = "SELINUXTYPE";
const SELINUX_DIR: &str = "/etc/selinux/";
const SELINUX_CONFIG: &str = "config";

#[derive(Debug, thiserror::Error)]
pub enum SELinuxError {
    #[error("Failed to set file label for SELinux: {0}")]
    SetFileLabel(String),
    #[error("Failed to lset file label for SELinux: {0}")]
    LSetFileLabel(String),
    #[error("Failed to get file label for SELinux: {0}")]
    FileLabel(String),
    #[error("Failed to get lfile label for SELinux: {0}")]
    LFileLabel(String),
    #[error("Failed to call is_proc_handle for SELinux: {0}")]
    IsProcHandle(String),
    #[error("Failed to call read_con_fd for SELinux: {0}")]
    ReadConFd(String),
    #[error("Failed to call read_con for SELinux: {0}")]
    ReadCon(String),
    #[error("Failed to call write_con for SELinux: {0}")]
    WriteCon(String),
    #[error("Failed to call class_index for SELinux: {0}")]
    ClassIndex(String),
    #[error("Failed to call peer_label for SELinux: {0}")]
    PeerLabel(String),
    #[error("Failed to call open_context_file for SELinux: {0}")]
    OpenContextFile(String),
}

pub struct SELinux {
    // for attr_path()
    have_thread_self: AtomicBool,
    attr_path_init_done: AtomicBool,

    // for selinuxfs
    selinuxfs_init_done: AtomicBool,
    selinuxfs: Option<String>,

    // for policy_root()
    policy_root_init_done: AtomicBool,
    policy_root: Option<String>,
}

impl Default for SELinux {
    fn default() -> Self {
        SELinux::new()
    }
}

impl SELinux {
    pub fn new() -> Self {
        SELinux {
            have_thread_self: AtomicBool::new(false),
            attr_path_init_done: AtomicBool::new(false),

            selinuxfs_init_done: AtomicBool::new(false),
            selinuxfs: None,

            policy_root_init_done: AtomicBool::new(false),
            policy_root: None,
        }
    }

    // function similar with policyRoot in go-selinux repo.
    // This function returns policy_root.
    // Directories under policy root has configuration files etc.
    fn policy_root(&mut self) -> String {
        // Avoiding code conflicts and ensuring thread-safe execution once only.
        if !self.policy_root_init_done.load(Ordering::SeqCst) {
            let policy_root_path = Self::read_config(SELINUX_TYPE_TAG);
            self.policy_root = Some(policy_root_path.clone());
            self.policy_root_init_done.store(true, Ordering::SeqCst);
            policy_root_path
        } else {
            self.policy_root
                .as_ref()
                .unwrap_or(&String::new())
                .to_string()
        }
    }

    // function similar with readConfig in go-selinux repo.
    // This function reads SELinux config file and returns the value with a specified key.
    fn read_config(target: &str) -> String {
        match File::open(format!("{}{}", SELINUX_DIR, SELINUX_CONFIG)) {
            Ok(file) => {
                let reader = BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    if line.is_empty() {
                        continue;
                    }
                    if (line.starts_with(';')) || (line.starts_with('#')) {
                        continue;
                    }
                    let fields: Vec<&str> = line.splitn(2, '=').collect();
                    if fields.len() < 2 {
                        return String::new();
                    }
                    if fields[0] == target {
                        return fields[1].to_owned();
                    }
                }
                String::new()
            }
            Err(_) => String::new(),
        }
    }

    // function similar with getEnabled in go-selinux repo.
    // get_enabled returns whether SELinux is enabled or not.
    pub fn get_enabled(&mut self) -> bool {
        let fs = Self::get_selinux_mountpoint(self);
        if fs == String::new() {
            return false;
        }
        match Self::current_label(self) {
            Ok(con) => {
                if con != "kernel" {
                    return true;
                }
                false
            }
            Err(_) => false,
        }
    }

    // function similar with verifySELinuxfsMount in go-selinux repo.
    // verify_selinux_fs_mount verifies if the specified mount point is
    // properly mounted as a writable SELinux filesystem.
    fn verify_selinux_fs_mount(mnt: &Path) -> bool {
        loop {
            match statfs::statfs(mnt) {
                Ok(stat) => {
                    // verify if the file is readonly or not
                    if !stat.flags().contains(statvfs::FsFlags::ST_RDONLY) {
                        return false;
                    }
                    // verify if the file is SELinux filesystem
                    return stat.filesystem_type() == statfs::SELINUX_MAGIC;
                }
                // check again if there is an issue while calling statfs
                Err(Errno::EAGAIN) | Err(Errno::EINTR) => continue,
                Err(_) => return false,
            }
        }
    }

    // function similar with findSELinuxfsMount in go-selinux repo.
    // find_selinux_fs_mount returns a next selinuxfs mount point found,
    // if there is one, or an empty string in case of EOF or error.
    fn find_selinux_fs_mount(s: &str) -> String {
        if !s.contains(" - selinuxfs ") {
            return String::new();
        }
        // Need to return the path like /sys/fs/selinux
        // example: 28 24 0:25 / /sys/fs/selinux rw,relatime - selinuxfs selinuxfs rw
        let m_pos = 5;
        let fields: Vec<&str> = s.splitn(m_pos + 1, ' ').collect();
        if fields.len() < m_pos + 1 {
            return String::new();
        }
        fields[m_pos - 1].to_string()
    }

    // function similar with findSELinuxfs in go-selinux repo.
    // find_selinux_fs finds the SELinux filesystem mount point.
    fn find_selinux_fs() -> String {
        // fast path: check the default mount first
        let selinux_fs_mount_path = Path::new(SELINUX_FS_MOUNT);
        if Self::verify_selinux_fs_mount(selinux_fs_mount_path) {
            return SELINUX_FS_MOUNT.to_string();
        }

        // check if selinuxfs is available before going the slow path
        let fs = fs::read_to_string("/proc/filesystems").unwrap_or_default();
        if !fs.contains("\tselinuxfs\n") {
            return String::new();
        }

        // continue reading until finding mount point
        loop {
            // slow path: try to find among the mounts
            match File::open("/proc/self/mountinfo") {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    for line in reader.lines().map_while(Result::ok) {
                        let mnt = Self::find_selinux_fs_mount(&line);
                        if mnt == String::new() {
                            continue;
                        }
                        let mnt_path = Path::new(&mnt);
                        if Self::verify_selinux_fs_mount(mnt_path) {
                            return mnt;
                        }
                    }
                }
                Err(_) => return String::new(),
            }
        }
    }

    // function similar with getSelinuxMountPoint in go-selinux repo.
    // This function returns the path to the mountpoint of an selinuxfs
    // filesystem or an empty string if no mountpoint is found. Selinuxfs is
    // a proc-like pseudo-filesystem that exposes the SELinux policy API to
    // processes. The existence of an seliuxfs mount is used to determine
    // whether SELinux is currently enabled or not.
    pub fn get_selinux_mountpoint(&mut self) -> String {
        // Avoiding code conflicts and ensuring thread-safe execution once only.
        if !self.selinuxfs_init_done.load(Ordering::SeqCst) {
            let selinuxfs_path = Self::find_selinux_fs();
            self.selinuxfs = Some(selinuxfs_path.clone());
            self.selinuxfs_init_done.store(true, Ordering::SeqCst);
            selinuxfs_path
        } else {
            self.selinuxfs
                .as_ref()
                .unwrap_or(&String::new())
                .to_string()
        }
    }

    // function similar with classIndex in go-selinux repo.
    // classIndex returns the int index for an object class in the loaded policy,
    // or -1 and an error.
    // If class is "file" or "dir", then return the corresponding index for selinux.
    pub fn class_index(&mut self, class: &str) -> Result<i64, SELinuxError> {
        let permpath = format!("class/{}/index", class);
        let selinux_mountpoint = Self::get_selinux_mountpoint(self);
        let indexpath = Path::new(&selinux_mountpoint).join(permpath);

        match fs::read_to_string(indexpath) {
            Ok(index_b) => match index_b.parse::<i64>() {
                Ok(index) => Ok(index),
                Err(e) => Err(SELinuxError::ClassIndex(e.to_string())),
            },
            Err(e) => Err(SELinuxError::ClassIndex(e.to_string())),
        }
    }

    // function similar with setFileLabel in go-selinux repo.
    // set_file_label sets the SELinux label for this path, following symlinks, or returns an error.
    pub fn set_file_label(fpath: &Path, label: &str) -> Result<(), SELinuxError> {
        if !fpath.exists() {
            return Err(SELinuxError::SetFileLabel(ERR_EMPTY_PATH.to_string()));
        }

        loop {
            match fpath.set_xattr(XATTR_NAME_SELINUX, label.as_bytes()) {
                Ok(_) => break,
                // When a system call is interrupted by a signal, it needs to be retried.
                Err(XattrError::EINTR(_)) => continue,
                Err(e) => {
                    return Err(SELinuxError::SetFileLabel(format!(
                        "set_xattr failed: {}",
                        e
                    )));
                }
            }
        }
        Ok(())
    }

    // function similar with lSetFileLabel in go-selinux repo.
    // lset_file_label sets the SELinux label for this path, not following symlinks,
    // or returns an error.
    pub fn lset_file_label(fpath: &Path, label: &str) -> Result<(), SELinuxError> {
        if !fpath.exists() {
            return Err(SELinuxError::LSetFileLabel(ERR_EMPTY_PATH.to_string()));
        }

        loop {
            match fpath.lset_xattr(XATTR_NAME_SELINUX, label.as_bytes()) {
                Ok(_) => break,
                // When a system call is interrupted by a signal, it needs to be retried.
                Err(XattrError::EINTR(_)) => continue,
                Err(e) => {
                    return Err(SELinuxError::LSetFileLabel(format!(
                        "lset_xattr failed: {}",
                        e
                    )));
                }
            }
        }
        Ok(())
    }

    // function similar with fileLabel in go-selinux repo.
    // fileLabel returns the SELinux label for this path, following symlinks,
    // or returns an error.
    pub fn file_label(fpath: &Path) -> Result<String, SELinuxError> {
        if !fpath.exists() {
            return Err(SELinuxError::FileLabel(ERR_EMPTY_PATH.to_string()));
        }
        fpath
            .get_xattr(XATTR_NAME_SELINUX)
            .map_err(|e| SELinuxError::FileLabel(e.to_string()))
    }

    // function similar with lFileLabel in go-selinux repo.
    // lfile_label returns the SELinux label for this path, not following symlinks,
    // or returns an error.
    pub fn lfile_label(fpath: &Path) -> Result<String, SELinuxError> {
        if !fpath.exists() {
            return Err(SELinuxError::LFileLabel(ERR_EMPTY_PATH.to_string()));
        }
        fpath
            .lget_xattr(XATTR_NAME_SELINUX)
            .map_err(|e| SELinuxError::LFileLabel(e.to_string()))
    }

    // function similar with setFSCreateLabel in go-selinux repo.
    // set_fscreate_label sets the default label the kernel which the kernel is using
    // for file system objects.
    pub fn set_fscreate_label(&mut self, label: &str) -> Result<usize, SELinuxError> {
        return Self::write_con(self, self.attr_path("fscreate").as_path(), label);
    }

    // function similar with fsCreateLabel in go-selinux repo.
    // fscreate_label returns the default label the kernel which the kernel is using
    // for file system objects created by this task. "" indicates default.
    pub fn fscreate_label(&self) -> Result<String, SELinuxError> {
        return Self::read_con(self.attr_path("fscreate").as_path());
    }

    // function similar with currentLabel in go-selinux repo.
    // current_label returns the SELinux label of the current process thread, or an error.
    pub fn current_label(&self) -> Result<String, SELinuxError> {
        return Self::read_con(self.attr_path("current").as_path());
    }

    // function similar with pidLabel in go-selinux repo.
    // pid_label returns the SELinux label of the given pid, or an error.
    pub fn pid_label(pid: i64) -> Result<String, SELinuxError> {
        let file_name = &format!("/proc/{}/attr/current", pid);
        let label = Path::new(file_name);
        Self::read_con(label)
    }

    // function similar with execLabel in go-selinux repo.
    // exec_label returns the SELinux label that the kernel will use for any programs
    // that are executed by the current process thread, or an error.
    pub fn exec_label(&self) -> Result<String, SELinuxError> {
        return Self::read_con(self.attr_path("exec").as_path());
    }

    // function similar with SetExecLabel in go-selinux repo.
    // set_exec_label sets the SELinux label that the kernel will use for any programs
    // that are executed by the current process thread, or an error.
    pub fn set_exec_label(&mut self, label: &str) -> Result<usize, SELinuxError> {
        Self::write_con(self, self.attr_path("exec").as_path(), label)
    }

    // function similar with SetTaskLabel in go-selinux repo.
    // set_task_label sets the SELinux label for the current thread, or an error.
    // This requires the dyntransition permission because this changes the context of current thread.
    pub fn set_task_label(&mut self, label: &str) -> Result<usize, SELinuxError> {
        Self::write_con(self, self.attr_path("current").as_path(), label)
    }

    // function similar with SetSocketLabel in go-selinux repo.
    // set_socket_label takes a process label and tells the kernel to assign the
    // label to the next socket that gets created.
    pub fn set_socket_label(&mut self, label: &str) -> Result<usize, SELinuxError> {
        Self::write_con(self, self.attr_path("sockcreate").as_path(), label)
    }

    // function similar with SocketLabel in go-selinux repo.
    // socket_label retrieves the current socket label setting.
    pub fn socket_label(&self) -> Result<String, SELinuxError> {
        return Self::read_con(self.attr_path("sockcreate").as_path());
    }

    // function similar with peerLabel in go-selinux repo.
    // peer_label retrieves the label of the client on the other side of a socket.
    pub fn peer_label<F: AsFd>(fd: F) -> Result<String, SELinuxError> {
        // getsockopt manipulate options for the socket referred to by the file descriptor.
        // https://man7.org/linux/man-pages/man2/getsockopt.2.html
        match getsockopt(&fd, PeerSec) {
            Ok(label) => match label.into_string() {
                Ok(label_str) => Ok(label_str),
                Err(e) => Err(SELinuxError::PeerLabel(e.to_string())),
            },
            Err(e) => Err(SELinuxError::PeerLabel(e.to_string())),
        }
    }

    // function similar with setKeyLabel in go-selinux repo.
    // set_key_label takes a process label and tells the kernel to assign the
    // label to the next kernel keyring that gets created.
    pub fn set_key_label(&mut self, label: &str) -> Result<usize, SELinuxError> {
        Self::write_con(self, Path::new(KEY_LABEL_PATH), label)
    }

    // function similar with KeyLabel in go-selinux repo.
    // key_label retrieves the current kernel keyring label setting
    pub fn key_label() -> Result<String, SELinuxError> {
        Self::read_con(Path::new(KEY_LABEL_PATH))
    }

    // function similar with reserveLabel in go-selinux repo.
    // reserve_label reserves the MLS/MCS level component of the specified label
    pub fn reserve_label(label: &str) {
        unimplemented!("not implemented yet")
    }

    // function similar with roFileLabel in go-selinux repo.
    // ro_file_label returns the specified SELinux readonly file label
    pub fn ro_file_label() {
        unimplemented!("not implemented yet")
    }

    // function similar with kvmContainerLabels in go-selinux repo.
    // kvm_container_labels returns the default processLabel and mountLabel to be used
    // for kvm containers by the calling process.
    pub fn kvm_container_labels() {
        unimplemented!("not implemented yet")
    }

    // function similar with initContainerLabels in go-selinux repo.
    // init_container_labels returns the default processLabel and file labels to be
    // used for containers running an init system like systemd by the calling process.
    pub fn init_container_labels() {
        unimplemented!("not implemented yet")
    }

    // function similar with containerLabels in go-selinux repo.
    // container_labels returns an allocated processLabel and fileLabel to be used for
    // container labeling by the calling process.
    pub fn container_labels() {
        unimplemented!("not implemented yet")
    }

    // function similar with PrivContainerMountLabel in go-selinux repo.
    // priv_container_mount_label returns mount label for privileged containers.
    pub fn priv_container_mount_label() {
        unimplemented!("not implemented yet")
    }

    // function similar with FormatMountLabel in go-selinux repo.
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
        Self::format_mount_label_by_type(src, mount_label, "context")
    }

    // function similar with FormatMountLabelByType in go-selinux repo.
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

    // function similar with openContextFile in go-selinux repo.
    // This function attempts to open a selinux context file, and if it fails, it tries to open another file
    // under policy root's directory.
    fn open_context_file(&mut self) -> Result<File, SELinuxError> {
        match File::open(CONTEXT_FILE) {
            Ok(file) => Ok(file),
            Err(_) => {
                let policy_path = Self::policy_root(self);
                let context_on_policy_root = Path::new(&policy_path)
                    .join("contexts")
                    .join("lxc_contexts");
                match File::open(context_on_policy_root) {
                    Ok(file) => Ok(file),
                    Err(e) => Err(SELinuxError::OpenContextFile(e.to_string())),
                }
            }
        }
    }

    // function similar with loadLabels in go-selinux repo.
    // This function loads context file and reads labels and stores it.
    fn load_labels(&mut self) -> HashMap<String, String> {
        let mut labels = HashMap::new();
        // The context file should have pairs of key and value like below.
        // ----------
        // SELINUXTYPE=targeted
        // SELINUX=enforcing
        // ----------
        if let Ok(file) = Self::open_context_file(self) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    continue;
                }
                let fields: Vec<&str> = line.splitn(2, '=').collect();
                if fields.len() != 2 {
                    continue;
                }
                let key = fields[0].trim().to_string();
                let value = fields[1].trim().to_string();
                labels.insert(key, value);
            }
        }
        labels
    }

    // function similar with writeCon in go-selinux repo.
    // write_con writes a specified value to a given file path, handling SELinux context.
    pub fn write_con(&mut self, fpath: &Path, val: &str) -> Result<usize, SELinuxError> {
        if fpath.as_os_str().is_empty() {
            return Err(SELinuxError::WriteCon(ERR_EMPTY_PATH.to_string()));
        }
        if val.is_empty() && !Self::get_enabled(self) {
            return Err(SELinuxError::WriteCon("SELinux is not enabled".to_string()));
        }

        let mut out = OpenOptions::new()
            .write(true)
            .create(false)
            .open(fpath)
            .map_err(|e| SELinuxError::WriteCon(format!("failed to open file: {}", e)))?;

        Self::is_proc_handle(&out)?;
        match out.write(val.as_bytes()) {
            Ok(u) => Ok(u),
            Err(e) => Err(SELinuxError::WriteCon(format!(
                "failed to write in file: {}",
                e
            ))),
        }
    }

    // function similar with isProcHandle in go-selinux repo.
    pub fn is_proc_handle(file: &File) -> Result<(), SELinuxError> {
        loop {
            match statfs::fstatfs(file.as_fd()) {
                Ok(stat) if stat.filesystem_type() == statfs::PROC_SUPER_MAGIC => break,
                Ok(_) => {
                    return Err(SELinuxError::IsProcHandle(format!(
                        "file {} is not on procfs",
                        file.as_raw_fd()
                    )));
                }
                Err(Errno::EINTR) => continue,
                Err(err) => {
                    return Err(SELinuxError::IsProcHandle(format!(
                        "fstatfs failed: {}",
                        err
                    )))
                }
            }
        }
        Ok(())
    }

    // function similar with readConFd in go-selinux repo.
    pub fn read_con_fd(file: &mut File) -> Result<String, SELinuxError> {
        let mut data = String::new();
        file.read_to_string(&mut data)
            .map_err(|e| SELinuxError::ReadConFd(e.to_string()))?;

        // Remove null bytes on the end of a file.
        let trimmed_data = data.trim_end_matches(char::from(0));
        Ok(trimmed_data.to_string())
    }

    // function similar with readCon in go-selinux repo.
    // read_con reads a label to a given file path, handling SELinux context.
    pub fn read_con(fpath: &Path) -> Result<String, SELinuxError> {
        if fpath.as_os_str().is_empty() {
            return Err(SELinuxError::ReadCon(ERR_EMPTY_PATH.to_string()));
        }
        let mut in_file = File::open(fpath)
            .map_err(|e| SELinuxError::ReadCon(format!("failed to open file: {}", e)))?;

        Self::is_proc_handle(&in_file)?;
        Self::read_con_fd(&mut in_file)
    }

    // function similar with attrPath in go-selinux repo.
    // attr_path determines the correct file path for accessing SELinux
    // attributes of a process or thread in a Linux environment.
    pub fn attr_path(&self, attr: &str) -> PathBuf {
        // Linux >= 3.17 provides this
        const THREAD_SELF_PREFIX: &str = "/proc/thread-self/attr";
        // Avoiding code conflicts and ensuring thread-safe execution once only.
        if !self.attr_path_init_done.load(Ordering::SeqCst) {
            let path = PathBuf::from(THREAD_SELF_PREFIX);
            let is_dir = path.is_dir();
            self.have_thread_self.store(is_dir, Ordering::SeqCst);
            self.attr_path_init_done.store(true, Ordering::SeqCst);
        }
        if self.have_thread_self.load(Ordering::SeqCst) {
            return PathBuf::from(&format!("{}/{}", THREAD_SELF_PREFIX, attr));
        }

        PathBuf::from(&format!("/proc/self/task/{}/attr/{}", gettid(), attr))
    }
}

#[cfg(test)]
mod tests {
    use crate::selinux::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;

    fn create_temp_file(content: &[u8], file_name: &str) {
        let path = Path::new(file_name);
        let mut file = File::create(&path).expect("Failed to create file");
        file.write_all(content).expect("Failed to write to file");
        file.sync_all().expect("Failed to sync file");
    }

    #[test]
    fn test_format_mount_label() {
        let src_array = ["", "src", "src"];
        let mount_label_array = ["foobar", "foobar", ""];
        let expected_array = ["context=\"foobar\"", "src,context=\"foobar\"", "src"];
        for (i, src) in src_array.iter().enumerate() {
            let mount_label = mount_label_array[i];
            let expected = expected_array[i];
            assert_eq!(SELinux::format_mount_label(src, mount_label), expected);
        }
    }

    #[test]
    fn test_format_mount_label_by_type() {
        let src_array = ["", "src", "src"];
        let mount_label_array = ["foobar", "foobar", ""];
        let context_array = ["fscontext", "fscontext", "rootcontext"];
        let expected_array = ["fscontext=\"foobar\"", "src,fscontext=\"foobar\"", "src"];
        for (i, src) in src_array.iter().enumerate() {
            let mount_label = mount_label_array[i];
            let context = context_array[i];
            let expected = expected_array[i];
            assert_eq!(
                SELinux::format_mount_label_by_type(src, mount_label, context),
                expected
            );
        }
    }

    #[test]
    fn test_read_con_fd() {
        let content_array: Vec<&[u8]> =
            vec![b"Hello, world\0", b"Hello, world\0\0\0", b"Hello,\0world"];
        let expected_array = ["Hello, world", "Hello, world", "Hello,\0world"];
        let file_name = "test.txt";
        for (i, content) in content_array.iter().enumerate() {
            let expected = expected_array[i];
            create_temp_file(content, file_name);
            // Need to open again to get read permission.
            let mut file = File::open(file_name).expect("Failed to open file");
            let result = SELinux::read_con_fd(&mut file).expect("Failed to read file");
            assert_eq!(result, expected);
            fs::remove_file(file_name).expect("Failed to remove test file");
        }
    }

    #[test]
    fn test_attr_path() {
        let selinux = SELinux::new();
        // Test with "/proc/thread-self/attr" path (Linux >= 3.17)
        let attr = "bar";
        let expected_name = &format!("/proc/thread-self/attr/{}", attr);
        let expected_path = Path::new(expected_name);
        let actual_path = selinux.attr_path(attr);
        assert_eq!(expected_path, actual_path);

        // Test with not having "/proc/thread-self/attr" path by setting HAVE_THREAD_SELF as false
        selinux.attr_path_init_done.store(true, Ordering::SeqCst);
        selinux.have_thread_self.store(false, Ordering::SeqCst);
        let thread_id = gettid();
        let expected_name = &format!("/proc/self/task/{}/attr/{}", thread_id, attr);
        let expected_path = Path::new(expected_name);
        let actual_path = selinux.attr_path(attr);
        assert_eq!(expected_path, actual_path);
    }

    #[test]
    fn test_is_proc_handle() {
        let filename_array = ["/proc/self/status", "/tmp/testfile"];
        let expected_array = [true, false];

        for (i, filename) in filename_array.iter().enumerate() {
            let expected_ok = expected_array[i];
            let path = Path::new(filename);
            let file = match File::open(path) {
                Ok(file) => file,
                Err(_) => {
                    create_temp_file(b"", filename);
                    File::open(path).expect("failed to open file")
                }
            };
            let result = SELinux::is_proc_handle(&file);
            if expected_ok {
                assert!(result.is_ok(), "Expected Ok, but got Err: {:?}", result);
            } else {
                assert!(result.is_err(), "Expected Err, but got Ok");
            }
        }
    }
}
