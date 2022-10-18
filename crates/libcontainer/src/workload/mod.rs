use anyhow::{Context, Result};
use oci_spec::runtime::Spec;

use self::wasmedge::WasmEdge;

pub mod wasmedge;

pub trait Executor {
    /// Executes the workload
    fn exec(spec: &Spec) -> Result<()>;
    /// Checks if the handler is able to handle the workload
    fn can_handle(spec: &Spec) -> Result<bool>;
    /// The name of the handler
    fn name() -> &'static str;
}
pub struct ExecutorManager {}

impl ExecutorManager {
    pub fn exec(spec: &Spec) -> Result<()> {
        return WasmEdge::exec(spec).context("wasmedge execution failed");
    }
}
