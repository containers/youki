//! During kernel initialization, a minimal replica of the ramfs filesystem is loaded, called rootfs.
//! Most systems mount another filesystem over it

#[allow(clippy::module_inception)]
pub(crate) mod rootfs;
pub use rootfs::RootFS;
