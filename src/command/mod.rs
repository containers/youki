//! Contains a wrapper of syscalls for unit tests

#[allow(clippy::module_inception)]
mod command;
pub mod linux;
pub mod test;

pub use command::Command;
