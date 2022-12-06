//! Implements Command trait for Linux systems
use std::ffi::{CStr, CString, OsStr};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::{any::Any, mem, path::Path, ptr};

use anyhow::{anyhow, bail, Context, Result};
use caps::{CapSet, CapsHashSet};
use libc::{c_char, setdomainname, uid_t};
use nix::fcntl;
use nix::{
    errno::Errno,
    fcntl::{open, OFlag},
    mount::{mount, umount2, MntFlags, MsFlags},
    sched::{unshare, CloneFlags},
    sys::stat::{mknod, Mode, SFlag},
    unistd,
    unistd::{chown, fchdir, pivot_root, setgroups, sethostname, Gid, Uid},
};
use syscalls::{syscall, Sysno, Sysno::close_range};

use oci_spec::runtime::LinuxRlimit;

use super::Syscall;
use crate::syscall::syscall::CloseRange;
use crate::{capabilities, utils};

// Constants used by mount_setattr(2).
pub const MOUNT_ATTR_RDONLY: u64 = 0x00000001; // Mount read-only.
pub const MOUNT_ATTR_NOSUID: u64 = 0x00000002; // Ignore suid and sgid bits.
pub const MOUNT_ATTR_NODEV: u64 = 0x00000004; // Disallow access to device special files.
pub const MOUNT_ATTR_NOEXEC: u64 = 0x00000008; // Disallow program execution.
pub const MOUNT_ATTR__ATIME: u64 = 0x00000070; // Setting on how atime should be updated.
pub const MOUNT_ATTR_RELATIME: u64 = 0x00000000; // - Update atime relative to mtime/ctime.
pub const MOUNT_ATTR_NOATIME: u64 = 0x00000010; // - Do not update access times.
pub const MOUNT_ATTR_STRICTATIME: u64 = 0x00000020; // - Always perform atime updates.
pub const MOUNT_ATTR_NODIRATIME: u64 = 0x00000080; // Do not update directory access times.
pub const MOUNT_ATTR_NOSYMFOLLOW: u64 = 0x00200000; // Prevents following symbolic links.
pub const AT_RECURSIVE: u32 = 0x00008000; // Change the mount properties of the entire mount tree.

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
// A structure used as te third argument of mount_setattr(2).
pub struct MountAttr {
    // Mount properties to set.
    pub attr_set: u64,
    // Mount properties to clear.
    pub attr_clr: u64,
    // Mount propagation type.
    pub propagation: u64,
    // User namespace file descriptor.
    pub userns_fd: u64,
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
        let open_fds = Self::get_open_fds().with_context(|| "failed to obtain opened fds")?;
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
        utils::ensure_procfs(Path::new(PROCFS_FD_PATH))
            .with_context(|| format!("{} is not the actual procfs", PROCFS_FD_PATH))?;

        let fds: Vec<i32> = fs::read_dir(PROCFS_FD_PATH)?
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
        let newroot = open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;

        // make the given path as the root directory for the container
        // see https://man7.org/linux/man-pages/man2/pivot_root.2.html, specially the notes
        // pivot root usually changes the root directory to first argument, and then mounts the original root
        // directory at second argument. Giving same path for both stacks mapping of the original root directory
        // above the new directory at the same path, then the call to umount unmounts the original root directory from
        // this path. This is done, as otherwise, we will need to create a separate temporary directory under the new root path
        // so we can move the original root there, and then unmount that. This way saves the creation of the temporary
        // directory to put original root directory.
        pivot_root(path, path)?;

        // Make the original root directory rslave to avoid propagating unmount event to the host mount namespace.
        // We should use MS_SLAVE not MS_PRIVATE according to https://github.com/opencontainers/runc/pull/1500.
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )?;

        // Unmount the original root directory which was stacked on top of new root directory
        // MNT_DETACH makes the mount point unavailable to new accesses, but waits till the original mount point
        // to be free of activity to actually unmount
        // see https://man7.org/linux/man-pages/man2/umount2.2.html for more information
        umount2("/", MntFlags::MNT_DETACH)?;
        // Change directory to root
        fchdir(newroot)?;
        Ok(())
    }

    /// Set namespace for process
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()> {
        nix::sched::setns(rawfd, nstype)?;
        Ok(())
    }

    /// set uid and gid for process
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()> {
        if let Err(e) = prctl::set_keep_capabilities(true) {
            bail!("set keep capabilities returned {}", e);
        };
        // args : real *id, effective *id, saved set *id respectively
        unistd::setresgid(gid, gid, gid)?;
        unistd::setresuid(uid, uid, uid)?;

        // if not the root user, reset capabilities to effective capabilities,
        // which are used by kernel to perform checks
        // see https://man7.org/linux/man-pages/man7/capabilities.7.html for more information
        if uid != Uid::from_raw(0) {
            capabilities::reset_effective(self)?;
        }
        if let Err(e) = prctl::set_keep_capabilities(false) {
            bail!("set keep capabilities returned {}", e);
        };
        Ok(())
    }

    /// Disassociate parts of execution context
    // see https://man7.org/linux/man-pages/man2/unshare.2.html for more information
    fn unshare(&self, flags: CloneFlags) -> Result<()> {
        unshare(flags)?;
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
        if let Err(e) = sethostname(hostname) {
            bail!("Failed to set {} as hostname. {:?}", hostname, e)
        }
        Ok(())
    }

    /// Sets domainname for process (see
    /// [setdomainname(2)](https://man7.org/linux/man-pages/man2/setdomainname.2.html)).
    fn set_domainname(&self, domainname: &str) -> Result<()> {
        let ptr = domainname.as_bytes().as_ptr() as *const c_char;
        let len = domainname.len();
        let res = unsafe { setdomainname(ptr, len) };

        match res {
            0 => Ok(()),
            -1 => bail!(
                "Failed to set {} as domainname. {}",
                domainname,
                std::io::Error::last_os_error()
            ),
            _ => bail!(
                "Failed to set {} as domainname. unexpected error occor.",
                domainname
            ),
        }
    }

    /// Sets resource limit for process
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()> {
        let rlim = &libc::rlimit {
            rlim_cur: rlimit.soft(),
            rlim_max: rlimit.hard(),
        };
        let res = unsafe { libc::setrlimit(rlimit.typ() as u32, rlim) };
        if let Err(e) = Errno::result(res).map(drop) {
            bail!("Failed to set {:?}. {:?}", rlimit.typ(), e)
        }
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
        unistd::chroot(path)?;

        Ok(())
    }

    fn mount(
        &self,
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> Result<()> {
        match mount(source, target, fstype, flags, data) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn symlink(&self, original: &Path, link: &Path) -> Result<()> {
        match symlink(original, link) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn mknod(&self, path: &Path, kind: SFlag, perm: Mode, dev: u64) -> Result<()> {
        match mknod(path, kind, perm, dev) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn chown(&self, path: &Path, owner: Option<Uid>, group: Option<Gid>) -> Result<()> {
        match chown(path, owner, group) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn set_groups(&self, groups: &[Gid]) -> Result<()> {
        match setgroups(groups) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn close_range(&self, preserve_fds: i32) -> Result<()> {
        let result = unsafe {
            syscall!(
                close_range,
                3 + preserve_fds as usize,
                usize::MAX,
                CloseRange::CLOEXEC.bits()
            )
        };

        match result {
            Ok(_) => Ok(()),
            Err(e) if e == syscalls::Errno::ENOSYS || e == syscalls::Errno::EINVAL => {
                // close_range was introduced in kernel 5.9 and CLOSEEXEC was introduced in
                // kernel 5.11. If the kernel is older we emulate close_range in userspace.
                Self::emulate_close_range(preserve_fds)
            }
            Err(e) => bail!(e),
        }
    }

    fn mount_setattr(
        &self,
        dirfd: RawFd,
        pathname: &Path,
        flags: u32,
        mount_attr: &MountAttr,
        size: libc::size_t,
    ) -> Result<()> {
        let path_pathbuf = pathname.to_path_buf();
        let path_str = path_pathbuf.to_str();
        let path_c_string = match path_str {
            Some(path_str) => CString::new(path_str)?,
            None => bail!("Invalid filename"),
        };
        let result = unsafe {
            syscall!(
                Sysno::mount_setattr,
                dirfd,
                path_c_string.as_ptr(),
                flags,
                mount_attr as *const MountAttr,
                size
            )
        };

        match result {
            Ok(_) => Ok(()),
            Err(e) => bail!(e),
        }
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
