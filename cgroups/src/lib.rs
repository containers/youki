//! Control groups provide a way of controlling groups of processes.
//! Examples: controlling resource limits, execution priority, measuring resource usage,
//! freezing, checkpointing and restarting groups of processes.
#[cfg(test)]
#[macro_use]
extern crate quickcheck;

pub mod common;
pub mod stats;
mod test;
pub mod v1;
pub mod v2;
