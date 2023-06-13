//! Control groups provide a way of controlling groups of processes.
//! Examples: controlling resource limits, execution priority, measuring resource usage,
//! freezing, checkpointing and restarting groups of processes.
#[cfg(test)]
#[macro_use]
extern crate quickcheck;

#[cfg(test)]
#[macro_use]
extern crate mockall;

mod test;

pub mod common;
pub mod stats;
#[cfg(feature = "systemd")]
pub mod systemd;
#[cfg(not(feature = "systemd"))]
#[path = "systemd_stub/mod.rs"]
pub mod systemd;
pub mod test_manager;
#[cfg(feature = "v1")]
pub mod v1;
#[cfg(not(feature = "v1"))]
#[path = "v1_stub/mod.rs"]
pub mod v1;
#[cfg(feature = "v2")]
pub mod v2;
#[cfg(not(feature = "v2"))]
#[path = "v2_stub/mod.rs"]
pub mod v2;
