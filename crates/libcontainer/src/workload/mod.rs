use anyhow::{Context, Result};
use oci_spec::runtime::Spec;

use self::default::DefaultExecutor;
#[cfg(feature = "wasm-wasmedge")]
use self::wasmedge::WasmEdgeExecutor;
#[cfg(feature = "wasm-wasmer")]
use self::wasmer::WasmerExecutor;

pub mod default;
#[cfg(feature = "wasm-wasmedge")]
pub mod wasmedge;
#[cfg(feature = "wasm-wasmer")]
pub mod wasmer;

static EMPTY: Vec<String> = Vec::new();

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
        #[cfg(feature = "wasm-wasmer")]
        if WasmerExecutor::can_handle(spec)? {
            return WasmerExecutor::exec(spec).context("wasmer execution failed");
        }

        #[cfg(feature = "wasm-wasmedge")]
        if WasmEdgeExecutor::can_handle(spec)? {
            return WasmEdgeExecutor::exec(spec).context("wasmedge execution failed");
        }

        DefaultExecutor::exec(spec).context("default execution failed")
    }
}
