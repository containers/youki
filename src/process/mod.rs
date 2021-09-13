//! Provides a thin wrapper around fork syscall,
//! with enums and functions specific to youki implemented

pub mod args;
pub mod channel;
pub mod fork;
pub(crate) mod init;
pub mod intermediate;
pub mod message;
