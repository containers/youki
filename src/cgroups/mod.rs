//! Control groups provide a way of controlling groups of processes
//! Like controlling resource limits, execution priority, measuring resource usage,
//! freezing, checkpointing and restarting groups of processes.

pub mod common;
mod test;
pub mod v1;
pub mod v2;
