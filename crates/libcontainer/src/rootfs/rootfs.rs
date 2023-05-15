use super::{
    device::Device,
    mount::{Mount, MountOptions},
    symlink::Symlink,
    utils::default_devices,
    Result, RootfsError,
};
use crate::{
    error::MissingSpecError,
    syscall::{syscall::create_syscall, Syscall},
};
use nix::mount::MsFlags;
use oci_spec::runtime::{Linux, Spec};
use std::path::Path;

/// Holds information about rootfs
pub struct RootFS {
    syscall: Box<dyn Syscall>,
}

impl Default for RootFS {
    fn default() -> Self {
        Self::new()
    }
}

impl RootFS {
    pub fn new() -> RootFS {
        RootFS {
            syscall: create_syscall(),
        }
    }

    pub fn prepare_rootfs(
        &self,
        spec: &Spec,
        rootfs: &Path,
        bind_devices: bool,
        cgroup_ns: bool,
    ) -> Result<()> {
        tracing::debug!("Prepare rootfs: {:?}", rootfs);
        let mut flags = MsFlags::MS_REC;
        let linux = spec
            .linux()
            .as_ref()
            .ok_or(MissingSpecError::MissingLinux)?;

        match linux.rootfs_propagation().as_deref() {
            Some("shared") => flags |= MsFlags::MS_SHARED,
            Some("private") => flags |= MsFlags::MS_PRIVATE,
            Some("slave" | "unbindable") | None => flags |= MsFlags::MS_SLAVE,
            Some(uknown) => {
                return Err(RootfsError::UnknownRootfsPropagation(uknown.to_string()));
            }
        }

        self.syscall
            .mount(None, Path::new("/"), None, flags, None)?;

        let mounter = Mount::new();

        mounter.make_parent_mount_private(rootfs)?;

        tracing::debug!("mount root fs {:?}", rootfs);
        self.syscall.mount(
            Some(rootfs),
            rootfs,
            None,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None,
        )?;

        let global_options = MountOptions {
            root: rootfs,
            label: linux.mount_label().as_deref(),
            cgroup_ns,
        };

        if let Some(mounts) = spec.mounts() {
            for mount in mounts {
                mounter.setup_mount(mount, &global_options)?;
            }
        }

        let symlinker = Symlink::new();
        symlinker.setup_kcore_symlink(rootfs)?;
        symlinker.setup_default_symlinks(rootfs)?;

        let devicer = Device::new();
        if let Some(added_devices) = linux.devices() {
            devicer.create_devices(
                rootfs,
                default_devices().iter().chain(added_devices),
                bind_devices,
            )
        } else {
            devicer.create_devices(rootfs, &default_devices(), bind_devices)
        }?;

        symlinker.setup_ptmx(rootfs)?;
        Ok(())
    }

    /// Change propagation type of rootfs as specified in spec.
    pub fn adjust_root_mount_propagation(&self, linux: &Linux) -> Result<()> {
        let rootfs_propagation = linux.rootfs_propagation().as_deref();
        let flags = match rootfs_propagation {
            Some("shared") => Some(MsFlags::MS_SHARED),
            Some("unbindable") => Some(MsFlags::MS_UNBINDABLE),
            _ => None,
        };

        if let Some(flags) = flags {
            tracing::debug!("make root mount {:?}", flags);
            self.syscall
                .mount(None, Path::new("/"), None, flags, None)?;
        }

        Ok(())
    }
}
