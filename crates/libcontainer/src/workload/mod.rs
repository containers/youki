use anyhow::{Context, Result};
use oci_spec::runtime::Spec;

use self::default::DefaultExecutor;
#[cfg(feature = "wasm-wasmer")]
use self::wasmer::WasmerExecutor;

pub mod default;
#[cfg(feature = "wasm-wasmer")]
pub mod wasmer;

static EMPTY: Vec<String> = Vec::new();

pub trait Executor {
    /// Executes the workload
    fn exec(&self, spec: &Spec) -> Result<()>;
    /// Checks if the handler is able to handle the workload
    fn can_handle(&self, spec: &Spec) -> Result<bool>;
    /// The name of the handler
    fn name(&self) -> &str;
}
pub struct CompositeExecutor {
    executors: Vec<Box<dyn Executor>>,
}

impl Executor for CompositeExecutor {
    fn exec(&self, spec: &Spec) -> Result<()> {
        for executor in &self.executors {
            if executor
                .can_handle(spec)
                .with_context(|| format!("executor {} failed on selection", executor.name()))?
            {
                let result = executor.exec(spec);
                if result.is_err() {
                    let error_msg = if executor.name() == "default" {
                        "executor default failed on exec. This might have been caused \
                            by another handler not being able to match on your request"
                            .to_string()
                    } else {
                        format!("executor {} failed on exec", executor.name())
                    };

                    return result.context(error_msg);
                } else {
                    return Ok(());
                }
            }
        }

        unreachable!("no suitable execution handler has been registered");
    }

    fn can_handle(&self, spec: &Spec) -> Result<bool> {
        Ok(self
            .executors
            .iter()
            .any(|h| h.can_handle(spec).unwrap_or_default()))
    }

    fn name(&self) -> &str {
        "composite"
    }
}

impl Default for CompositeExecutor {
    fn default() -> Self {
        let handlers: Vec<Box<dyn Executor>> = vec![
            #[cfg(feature = "wasm-wasmer")]
            Box::new(WasmerExecutor {}),
            Box::new(DefaultExecutor {}),
        ];

        Self {
            executors: handlers,
        }
    }
}
