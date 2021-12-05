//! Control groups provide a way of controlling groups of processes.
//! Examples: controlling resource limits, execution priority, measuring resource usage,
//! freezing, checkpointing and restarting groups of processes.
#[cfg(test)]
#[macro_use]
extern crate quickcheck;
mod test;

pub mod common;
pub mod stats;
#[cfg(feature = "systemd")]
pub mod systemd;
pub mod test_manager;
#[cfg(feature = "v1")]
pub mod v1;
#[cfg(feature = "v2")]
pub mod v2;
