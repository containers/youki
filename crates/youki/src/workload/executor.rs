use libcontainer::workload::{default::DefaultExecutor, Executor};

pub fn default_executors() -> Vec<Box<dyn Executor>> {
    vec![
        #[cfg(feature = "wasm-wasmer")]
        Box::new(super::wasmer::WasmerExecutor::default()),
        #[cfg(feature = "wasm-wasmedge")]
        Box::new(super::wasmedge::WasmEdgeExecutor::default()),
        #[cfg(feature = "wasm-wasmtime")]
        Box::new(super::wasmtime::WasmtimeExecutor::default()),
        Box::new(DefaultExecutor::default()),
    ]
}
