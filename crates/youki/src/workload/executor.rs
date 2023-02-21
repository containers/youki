use libcontainer::workload::{default::DefaultExecutor, Executor};

pub fn default_executors() -> Vec<Box<dyn Executor>> {
    vec![
        #[cfg(feature = "wasm-wasmer")]
        Box::<super::wasmer::WasmerExecutor>::default(),
        #[cfg(feature = "wasm-wasmedge")]
        Box::<super::wasmedge::WasmEdgeExecutor>::default(),
        #[cfg(feature = "wasm-wasmtime")]
        Box::<super::wasmtime::WasmtimeExecutor>::default(),
        Box::<DefaultExecutor>::default(),
    ]
}
