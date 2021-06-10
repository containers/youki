//! Implements Command trait for Linux systems
use std::{any::Any, path::Path};

use anyhow::{bail, Result};
use caps::{errors::CapsError, CapSet, CapsHashSet};
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

use super::Command;
use crate::capabilities;

/// Empty structure to implement Command trait for
#[derive(Clone)]
pub struct LinuxCommand;

impl Command for LinuxCommand {
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
        caps::set(None, cset, value)
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
}
