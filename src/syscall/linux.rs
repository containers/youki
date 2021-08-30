//! Implements Command trait for Linux systems
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;
use std::{any::Any, mem, path::Path, ptr};

use anyhow::{bail, Result};
use caps::{errors::CapsError, CapSet, Capability, CapsHashSet};
use libc::{c_char, uid_t};
use nix::{
    errno::Errno,
    unistd::{fchdir, pivot_root, sethostname},
};
use nix::{fcntl::open, sched::CloneFlags};
use nix::{
    fcntl::OFlag,
    unistd::{Gid, Uid},
};
use nix::{
    mount::{umount2, MntFlags},
    unistd,
};
use nix::{sched::unshare, sys::stat::Mode};

use oci_spec::LinuxRlimit;

use super::Syscall;
use crate::capabilities;

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
}

impl Syscall for LinuxSyscall {
    /// To enable dynamic typing,
    /// see https://doc.rust-lang.org/std/any/index.html for more information
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
    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError> {
        match cset {
            // caps::set cannot set capabilities in bounding set,
            // so we do it differently
            CapSet::Bounding => {
                // get all capabilities
                let all = caps::all();
                // the difference will give capabilities
                // which are to be unset
                // for each such =, drop that capability
                // after this, only those which are to be set will remain set
                for c in all.difference(value) {
                    match c {
                        Capability::CAP_PERFMON
                        | Capability::CAP_CHECKPOINT_RESTORE
                        | Capability::CAP_BPF => {
                            log::warn!("{:?} is not supported.", c);
                            continue;
                        }
                        _ => caps::drop(None, CapSet::Bounding, *c)?,
                    }
                }
                Ok(())
            }
            _ => caps::set(None, cset, value),
        }
    }

    /// Sets hostname for process
    fn set_hostname(&self, hostname: &str) -> Result<()> {
        if let Err(e) = sethostname(hostname) {
            bail!("Failed to set {} as hostname. {:?}", hostname, e)
        }
        Ok(())
    }

    /// Sets resource limit for process
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()> {
        let rlim = &libc::rlimit {
            rlim_cur: rlimit.soft,
            rlim_max: rlimit.hard,
        };
        let res = unsafe { libc::setrlimit(rlimit.typ as u32, rlim) };
        if let Err(e) = Errno::result(res).map(drop) {
            bail!("Failed to set {:?}. {:?}", rlimit.typ, e)
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
}
