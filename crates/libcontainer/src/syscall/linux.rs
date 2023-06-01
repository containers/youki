//! Implements Command trait for Linux systems
use caps::{CapSet, CapsHashSet};
use libc::{c_char, setdomainname, uid_t};
use nix::fcntl;
use nix::{
    fcntl::{open, OFlag},
    mount::{mount, umount2, MntFlags, MsFlags},
    sched::{unshare, CloneFlags},
    sys::stat::{mknod, Mode, SFlag},
    unistd,
    unistd::{chown, fchdir, pivot_root, setgroups, sethostname, Gid, Uid},
};
use oci_spec::runtime::LinuxRlimit;
use std::ffi::{CStr, CString, OsStr};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::os::unix::io::RawFd;
use std::str::FromStr;
use std::sync::Arc;
use std::{any::Any, mem, path::Path, ptr};

use super::{Result, Syscall, SyscallError};
use crate::{capabilities, utils};

// Flags used in mount_setattr(2).
// see https://man7.org/linux/man-pages/man2/mount_setattr.2.html.
pub const AT_RECURSIVE: u32 = 0x00008000; // Change the mount properties of the entire mount tree.
pub const MOUNT_ATTR__ATIME: u64 = 0x00000070; // Setting on how atime should be updated.
const MOUNT_ATTR_RDONLY: u64 = 0x00000001;
const MOUNT_ATTR_NOSUID: u64 = 0x00000002;
const MOUNT_ATTR_NODEV: u64 = 0x00000004;
const MOUNT_ATTR_NOEXEC: u64 = 0x00000008;
const MOUNT_ATTR_RELATIME: u64 = 0x00000000;
const MOUNT_ATTR_NOATIME: u64 = 0x00000010;
const MOUNT_ATTR_STRICTATIME: u64 = 0x00000020;
const MOUNT_ATTR_NODIRATIME: u64 = 0x00000080;
const MOUNT_ATTR_NOSYMFOLLOW: u64 = 0x00200000;

/// Constants used by mount_setattr(2).
pub enum MountAttrOption {
    /// Mount read-only.
    MountArrtRdonly(bool, u64),

    /// Ignore suid and sgid bits.
    MountAttrNosuid(bool, u64),

    /// Disallow access to device special files.
    MountAttrNodev(bool, u64),

    /// Disallow program execution.
    MountAttrNoexec(bool, u64),

    /// Setting on how atime should be updated.
    MountAttrAtime(bool, u64),

    /// Update atime relative to mtime/ctime.
    MountAttrRelatime(bool, u64),

    /// Do not update access times.
    MountAttrNoatime(bool, u64),

    /// Always perform atime updates.
    MountAttrStrictAtime(bool, u64),

    /// Do not update directory access times.
    MountAttrNoDiratime(bool, u64),

    /// Prevents following symbolic links.
    MountAttrNosymfollow(bool, u64),
}

impl FromStr for MountAttrOption {
    type Err = SyscallError;

    fn from_str(option: &str) -> std::result::Result<Self, Self::Err> {
        match option {
            "rro" => Ok(MountAttrOption::MountArrtRdonly(false, MOUNT_ATTR_RDONLY)),
            "rrw" => Ok(MountAttrOption::MountArrtRdonly(true, MOUNT_ATTR_RDONLY)),
            "rnosuid" => Ok(MountAttrOption::MountAttrNosuid(false, MOUNT_ATTR_NOSUID)),
            "rsuid" => Ok(MountAttrOption::MountAttrNosuid(true, MOUNT_ATTR_NOSUID)),
            "rnodev" => Ok(MountAttrOption::MountAttrNodev(false, MOUNT_ATTR_NODEV)),
            "rdev" => Ok(MountAttrOption::MountAttrNodev(true, MOUNT_ATTR_NODEV)),
            "rnoexec" => Ok(MountAttrOption::MountAttrNoexec(false, MOUNT_ATTR_NOEXEC)),
            "rexec" => Ok(MountAttrOption::MountAttrNoexec(true, MOUNT_ATTR_NOEXEC)),
            "rnodiratime" => Ok(MountAttrOption::MountAttrNoDiratime(
                false,
                MOUNT_ATTR_NODIRATIME,
            )),
            "rdiratime" => Ok(MountAttrOption::MountAttrNoDiratime(
                true,
                MOUNT_ATTR_NODIRATIME,
            )),
            "rrelatime" => Ok(MountAttrOption::MountAttrRelatime(
                false,
                MOUNT_ATTR_RELATIME,
            )),
            "rnorelatime" => Ok(MountAttrOption::MountAttrRelatime(
                true,
                MOUNT_ATTR_RELATIME,
            )),
            "rnoatime" => Ok(MountAttrOption::MountAttrNoatime(false, MOUNT_ATTR_NOATIME)),
            "ratime" => Ok(MountAttrOption::MountAttrNoatime(true, MOUNT_ATTR_NOATIME)),
            "rstrictatime" => Ok(MountAttrOption::MountAttrStrictAtime(
                false,
                MOUNT_ATTR_STRICTATIME,
            )),
            "rnostrictatime" => Ok(MountAttrOption::MountAttrStrictAtime(
                true,
                MOUNT_ATTR_STRICTATIME,
            )),
            "rnosymfollow" => Ok(MountAttrOption::MountAttrNosymfollow(
                false,
                MOUNT_ATTR_NOSYMFOLLOW,
            )),
            "rsymfollow" => Ok(MountAttrOption::MountAttrNosymfollow(
                true,
                MOUNT_ATTR_NOSYMFOLLOW,
            )),
            // No support for MOUNT_ATTR_IDMAP yet (needs UserNS FD)
            _ => Err(SyscallError::UnexpectedMountAttrOption(option.to_string())),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
/// A structure used as te third argument of mount_setattr(2).
pub struct MountAttr {
    /// Mount properties to set.
    pub attr_set: u64,

    /// Mount properties to clear.
    pub attr_clr: u64,

    /// Mount propagation type.
    pub propagation: u64,

    /// User namespace file descriptor.
    pub userns_fd: u64,
}

impl MountAttr {
    /// Return MountAttr with the flag raised.
    /// This function is used in test code.
    pub fn all() -> Self {
        MountAttr {
            attr_set: MOUNT_ATTR_RDONLY
                | MOUNT_ATTR_NOSUID
                | MOUNT_ATTR_NODEV
                | MOUNT_ATTR_NOEXEC
                | MOUNT_ATTR_NODIRATIME
                | MOUNT_ATTR_RELATIME
                | MOUNT_ATTR_NOATIME
                | MOUNT_ATTR_STRICTATIME
                | MOUNT_ATTR_NOSYMFOLLOW,
            attr_clr: MOUNT_ATTR_RDONLY
                | MOUNT_ATTR_NOSUID
                | MOUNT_ATTR_NODEV
                | MOUNT_ATTR_NOEXEC
                | MOUNT_ATTR_NODIRATIME
                | MOUNT_ATTR_RELATIME
                | MOUNT_ATTR_NOATIME
                | MOUNT_ATTR_STRICTATIME
                | MOUNT_ATTR_NOSYMFOLLOW
                | MOUNT_ATTR__ATIME,
            propagation: 0,
            userns_fd: 0,
        }
    }
}

/// Empty structure to implement Command trait for
#[derive(Clone)]
pub struct LinuxSyscall;

impl LinuxSyscall {
    unsafe fn from_raw_buf<'a, T>(p: *const c_char) -> T
    where
        T: From<&'a OsStr>,
    {
        T::from(OsStr::from_bytes(CStr::from_ptr(p).to_bytes()))
    }

    /// Reads data from the `c_passwd` and returns it as a `User`.
    unsafe fn passwd_to_user(passwd: libc::passwd) -> Arc<OsStr> {
        let name: Arc<OsStr> = Self::from_raw_buf(passwd.pw_name);
        name
    }

    fn emulate_close_range(preserve_fds: i32) -> Result<()> {
        let open_fds = Self::get_open_fds()?;
        // Include stdin, stdout, and stderr for fd 0, 1, and 2 respectively.
        let min_fd = preserve_fds + 3;
        let to_be_cleaned_up_fds: Vec<i32> = open_fds
            .iter()
            .filter_map(|&fd| if fd >= min_fd { Some(fd) } else { None })
            .collect();

        to_be_cleaned_up_fds.iter().for_each(|&fd| {
            // Intentionally ignore errors here -- the cases where this might fail
            // are basically file descriptors that have already been closed.
            let _ = fcntl::fcntl(fd, fcntl::F_SETFD(fcntl::FdFlag::FD_CLOEXEC));
        });

        Ok(())
    }

    // Get a list of open fds for the calling process.
    fn get_open_fds() -> Result<Vec<i32>> {
        const PROCFS_FD_PATH: &str = "/proc/self/fd";
        utils::ensure_procfs(Path::new(PROCFS_FD_PATH)).map_err(|err| {
            tracing::error!(?err, "failed to ensure /proc is mounted");
            match err {
                utils::EnsureProcfsError::Nix(err) => SyscallError::Nix(err),
                utils::EnsureProcfsError::IO(err) => SyscallError::IO(err),
            }
        })?;

        let fds: Vec<i32> = fs::read_dir(PROCFS_FD_PATH)
            .map_err(|err| {
                tracing::error!(?err, "failed to read /proc/self/fd");
                err
            })?
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(_) => None,
            })
            .filter_map(|path| path.file_name().map(|file_name| file_name.to_owned()))
            .filter_map(|file_name| file_name.to_str().map(String::from))
            .filter_map(|file_name| -> Option<i32> {
                // Convert the file name from string into i32. Since we are looking
                // at /proc/<pid>/fd, anything that's not a number (i32) can be
                // ignored. We are only interested in opened fds.
                match file_name.parse() {
                    Ok(fd) => Some(fd),
                    Err(_) => None,
                }
            })
            .collect();

        Ok(fds)
    }
}

impl Syscall for LinuxSyscall {
    /// To enable dynamic typing,
    /// see <https://doc.rust-lang.org/std/any/index.html> for more information
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Function to set given path as root path inside process
    fn pivot_rootfs(&self, path: &Path) -> Result<()> {
        // open the path as directory and read only
        let newroot =
            open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty()).map_err(|errno| {
                tracing::error!(?errno, ?path, "failed to open the new root for pivot root");
                errno
            })?;

        // make the given path as the root directory for the container
        // see https://man7.org/linux/man-pages/man2/pivot_root.2.html, specially the notes
        // pivot root usually changes the root directory to first argument, and then mounts the original root
        // directory at second argument. Giving same path for both stacks mapping of the original root directory
        // above the new directory at the same path, then the call to umount unmounts the original root directory from
        // this path. This is done, as otherwise, we will need to create a separate temporary directory under the new root path
        // so we can move the original root there, and then unmount that. This way saves the creation of the temporary
        // directory to put original root directory.
        pivot_root(path, path).map_err(|errno| {
            tracing::error!(?errno, ?path, "failed to pivot root to");
            errno
        })?;

        // Make the original root directory rslave to avoid propagating unmount event to the host mount namespace.
        // We should use MS_SLAVE not MS_PRIVATE according to https://github.com/opencontainers/runc/pull/1500.
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|errno| {
            tracing::error!(?errno, "failed to make original root directory rslave");
            errno
        })?;

        // Unmount the original root directory which was stacked on top of new root directory
        // MNT_DETACH makes the mount point unavailable to new accesses, but waits till the original mount point
        // to be free of activity to actually unmount
        // see https://man7.org/linux/man-pages/man2/umount2.2.html for more information
        umount2("/", MntFlags::MNT_DETACH).map_err(|errno| {
            tracing::error!(?errno, "failed to unmount old root directory");
            errno
        })?;
        // Change directory to the new root
        fchdir(newroot).map_err(|errno| {
            tracing::error!(?errno, ?newroot, "failed to change directory to new root");
            errno
        })?;

        Ok(())
    }

    /// Set namespace for process
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()> {
        nix::sched::setns(rawfd, nstype)?;
        Ok(())
    }

    /// set uid and gid for process
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()> {
        prctl::set_keep_capabilities(true).map_err(|errno| {
            tracing::error!(?errno, "failed to set keep capabilities to true");
            nix::errno::from_i32(errno)
        })?;
        // args : real *id, effective *id, saved set *id respectively
        unistd::setresgid(gid, gid, gid).map_err(|err| {
            tracing::error!(
                ?err,
                ?gid,
                "failed to set real, effective and saved set gid"
            );
            err
        })?;
        unistd::setresuid(uid, uid, uid).map_err(|err| {
            tracing::error!(
                ?err,
                ?uid,
                "failed to set real, effective and saved set uid"
            );
            err
        })?;

        // if not the root user, reset capabilities to effective capabilities,
        // which are used by kernel to perform checks
        // see https://man7.org/linux/man-pages/man7/capabilities.7.html for more information
        if uid != Uid::from_raw(0) {
            capabilities::reset_effective(self)?;
        }
        prctl::set_keep_capabilities(false).map_err(|errno| {
            tracing::error!(?errno, "failed to set keep capabilities to false");
            nix::errno::from_i32(errno)
        })?;
        Ok(())
    }

    /// Disassociate parts of execution context
    // see https://man7.org/linux/man-pages/man2/unshare.2.html for more information
    fn unshare(&self, flags: CloneFlags) -> Result<()> {
        unshare(flags).map_err(|err| {
            tracing::error!(?err, ?flags, "failed to unshare");
            err
        })?;

        Ok(())
    }

    /// Set capabilities for container process
    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<()> {
        match cset {
            // caps::set cannot set capabilities in bounding set,
            // so we do it differently
            CapSet::Bounding => {
                // get all capabilities
                let all = caps::read(None, CapSet::Bounding)?;
                // the difference will give capabilities
                // which are to be unset
                // for each such =, drop that capability
                // after this, only those which are to be set will remain set
                for c in all.difference(value) {
                    caps::drop(None, CapSet::Bounding, *c)?
                }
            }
            _ => {
                caps::set(None, cset, value)?;
            }
        }
        Ok(())
    }

    /// Sets hostname for process
    fn set_hostname(&self, hostname: &str) -> Result<()> {
        sethostname(hostname).map_err(|err| {
            tracing::error!(?hostname, "failed to set hostname");
            err
        })?;
        Ok(())
    }

    /// Sets domainname for process (see
    /// [setdomainname(2)](https://man7.org/linux/man-pages/man2/setdomainname.2.html)).
    fn set_domainname(&self, domainname: &str) -> Result<()> {
        let ptr = domainname.as_bytes().as_ptr() as *const c_char;
        let len = domainname.len();
        match unsafe { setdomainname(ptr, len) } {
            0 => Ok(()),
            -1 => {
                tracing::error!(?domainname, "failed to set domainname");
                Err(nix::Error::last())
            }

            _ => {
                tracing::error!(?domainname, "failed to set domainname for unknown reason");
                Err(nix::Error::UnknownErrno)
            }
        }?;

        Ok(())
    }

    /// Sets resource limit for process
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()> {
        let rlim = &libc::rlimit {
            rlim_cur: rlimit.soft(),
            rlim_max: rlimit.hard(),
        };

        // Change for musl libc based on seccomp needs
        #[cfg(not(target_env = "musl"))]
        let res = unsafe { libc::setrlimit(rlimit.typ() as u32, rlim) };
        #[cfg(target_env = "musl")]
        let res = unsafe { libc::setrlimit(rlimit.typ() as i32, rlim) };

        match res {
            0 => Ok(()),
            -1 => {
                tracing::error!(?res, rlimit = ?rlimit.typ(), "failed to set rlimit");
                Err(SyscallError::Nix(nix::Error::last()))
            }
            _ => {
                tracing::error!(?res, rlimit = ?rlimit.typ(), "failed to set rlimit for unknown reason");
                Err(SyscallError::Nix(nix::Error::UnknownErrno))
            }
        }?;

        Ok(())
    }

    // taken from https://crates.io/crates/users
    fn get_pwuid(&self, uid: uid_t) -> Option<Arc<OsStr>> {
        let mut passwd = unsafe { mem::zeroed::<libc::passwd>() };
        let mut buf = vec![0; 2048];
        let mut result = ptr::null_mut::<libc::passwd>();

        loop {
            let r = unsafe {
                libc::getpwuid_r(uid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result)
            };

            if r != libc::ERANGE {
                break;
            }

            let newsize = buf.len().checked_mul(2)?;
            buf.resize(newsize, 0);
        }

        if result.is_null() {
            // There is no such user, or an error has occurred.
            // errno gets set if there’s an error.
            return None;
        }

        if result != &mut passwd {
            // The result of getpwuid_r should be its input passwd.
            return None;
        }

        let user = unsafe { Self::passwd_to_user(result.read()) };
        Some(user)
    }

    fn chroot(&self, path: &Path) -> Result<()> {
        unistd::chroot(path).map_err(|err| {
            tracing::error!(?err, ?path, "failed to chroot");
            err
        })?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn mount(
        &self,
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> Result<()> {
        mount(source, target, fstype, flags, data)
        .map_err(|err| {
            // ENOTDIR and ENOENT are sometimes expected when mounting, so we
            // don't log anything. Instead, the caller should decide whether to
            // log or not.  Other error codes are unexpected, so we log them.
            match err {
                nix::Error::ENOTDIR | nix::Error::ENOENT => {},
                _ => {
                    tracing::error!(
                        "failed to mount {source:?} to {target:?} with fstype {fstype:?}, flags {flags:?}, data {data:?}: {err}",
                    );
                }
            };

            err
        })?;

        Ok(())
    }

    fn symlink(&self, original: &Path, link: &Path) -> Result<()> {
        symlink(original, link).map_err(|err| {
            tracing::error!("failed to create symlink from {original:?} to {link:?}: {err}");
            err
        })?;

        Ok(())
    }

    fn mknod(&self, path: &Path, kind: SFlag, perm: Mode, dev: u64) -> Result<()> {
        mknod(path, kind, perm, dev).map_err(|errno| {
            tracing::error!(
                "failed to mknod {path:?}, kind {kind:?}, perm {perm:?}, dev {dev:?}: {errno}"
            );
            errno
        })?;

        Ok(())
    }

    fn chown(&self, path: &Path, owner: Option<Uid>, group: Option<Gid>) -> Result<()> {
        chown(path, owner, group).map_err(|errno| {
            tracing::error!("failed to chown {path:?} to {owner:?}:{group:?}: {errno}");
            errno
        })?;

        Ok(())
    }

    fn set_groups(&self, groups: &[Gid]) -> Result<()> {
        setgroups(groups)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn close_range(&self, preserve_fds: i32) -> Result<()> {
        match unsafe {
            libc::syscall(
                libc::SYS_close_range,
                3 + preserve_fds,
                libc::c_int::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            )
        } {
            0 => Ok(()),
            -1 => {
                match nix::errno::Errno::last() {
                    nix::errno::Errno::ENOSYS | nix::errno::Errno::EINVAL => {
                        // close_range was introduced in kernel 5.9 and CLOSEEXEC was introduced in
                        // kernel 5.11. If the kernel is older we emulate close_range in userspace.
                        Self::emulate_close_range(preserve_fds)
                    }
                    e => {
                        tracing::error!(errno = ?e, "failed to close_range");
                        Err(SyscallError::Nix(e))
                    }
                }
            }
            _ => {
                tracing::error!("failed to close_range unknown failure");
                Err(SyscallError::Nix(nix::errno::Errno::UnknownErrno))
            }
        }?;

        Ok(())
    }

    fn mount_setattr(
        &self,
        dirfd: RawFd,
        pathname: &Path,
        flags: u32,
        mount_attr: &MountAttr,
        size: libc::size_t,
    ) -> Result<()> {
        let path_c_string = pathname
            .to_path_buf()
            .to_str()
            .map(CString::new)
            .ok_or_else(|| {
                tracing::error!(path = ?pathname, "failed to convert path to string");
                nix::Error::EINVAL
            })?
            .map_err(|err| {
                tracing::error!(path = ?pathname, ?err, "failed to convert path to string");
                nix::Error::EINVAL
            })?;

        match unsafe {
            libc::syscall(
                libc::SYS_mount_setattr,
                dirfd,
                path_c_string.as_ptr(),
                flags,
                mount_attr as *const MountAttr,
                size,
            )
        } {
            0 => Ok(()),
            -1 => Err(nix::Error::last()),
            _ => Err(nix::Error::UnknownErrno),
        }?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note: We have to run these tests here as serial. The main issue is that
    // these tests has a dependency on the system state. The
    // cleanup_file_descriptors test is especially evil when running with other
    // tests because it would ran around close down different fds.

    use std::{fs, os::unix::prelude::AsRawFd};

    use anyhow::{bail, Context, Result};
    use nix::{fcntl, sys, unistd};
    use serial_test::serial;

    use crate::syscall::Syscall;

    use super::LinuxSyscall;

    #[test]
    #[serial]
    fn test_get_open_fds() -> Result<()> {
        let file = fs::File::open("/dev/null")?;
        let fd = file.as_raw_fd();
        let open_fds = LinuxSyscall::get_open_fds()?;

        if !open_fds.iter().any(|&v| v == fd) {
            bail!("failed to find the opened dev null fds: {:?}", open_fds);
        }

        // explicitly close the file before the test case returns.
        drop(file);

        // The stdio fds should also be contained in the list of opened fds.
        if !vec![0, 1, 2]
            .iter()
            .all(|&stdio_fd| open_fds.iter().any(|&open_fd| open_fd == stdio_fd))
        {
            bail!("failed to find the stdio fds: {:?}", open_fds);
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn test_close_range_userspace() -> Result<()> {
        // Open a fd without the CLOEXEC flag. Rust automatically adds the flag,
        // so we use fcntl::open here for more control.
        let fd = fcntl::open("/dev/null", fcntl::OFlag::O_RDWR, sys::stat::Mode::empty())?;
        LinuxSyscall::emulate_close_range(0).context("failed to clean up the fds")?;

        let fd_flag = fcntl::fcntl(fd, fcntl::F_GETFD)?;
        if (fd_flag & fcntl::FdFlag::FD_CLOEXEC.bits()) == 0 {
            bail!("CLOEXEC flag is not set correctly");
        }

        unistd::close(fd)?;
        Ok(())
    }

    #[test]
    #[serial]
    fn test_close_range_native() -> Result<()> {
        let fd = fcntl::open("/dev/null", fcntl::OFlag::O_RDWR, sys::stat::Mode::empty())?;
        let syscall = LinuxSyscall {};
        syscall
            .close_range(0)
            .context("failed to clean up the fds")?;

        let fd_flag = fcntl::fcntl(fd, fcntl::F_GETFD)?;
        if (fd_flag & fcntl::FdFlag::FD_CLOEXEC.bits()) == 0 {
            bail!("CLOEXEC flag is not set correctly");
        }

        unistd::close(fd)?;
        Ok(())
    }
}
