use libcontainer::workload::{Executor, ExecutorError};
use oci_spec::runtime::Spec;

pub fn default_executor() -> Executor {
    return Box::new(|spec: &Spec| -> Result<(), ExecutorError> {
        #[cfg(feature = "wasm-wasmer")]
        match super::wasmer::get_executor()(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmedge")]
        match super::wasmedge::get_executor()(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmtime")]
        match super::wasmtime::get_executor()(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }

        libcontainer::workload::default::get_executor()(spec)
    });
}
