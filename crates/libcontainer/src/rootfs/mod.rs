//! During kernel initialization, a minimal replica of the ramfs filesystem is loaded, called rootfs.
//! Most systems mount another filesystem over it

#[allow(clippy::module_inception)]
pub(crate) mod rootfs;
pub use rootfs::RootFS;

pub(super) mod device;
pub(super) mod mount;
pub(super) mod symlink;
pub(super) mod utils;
