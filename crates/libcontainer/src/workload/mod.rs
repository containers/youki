use anyhow::{Result, Context};
use oci_spec::runtime::Spec;

use self::{default::DefaultExecHandler};
#[cfg(feature = "wasm-wasmer")]
use self::{wasmer::WasmerExecHandler};

pub mod default;
#[cfg(feature = "wasm-wasmer")]
pub mod wasmer;

static EMPTY: Vec<String> = Vec::new();

pub trait ExecHandler {
    /// The name of the handler
    fn name(&self) -> &str;
    /// Executes the workload
    fn exec(&self, spec: &Spec) -> Result<()>;
    /// Checks if the handler is able to handle the workload
    fn can_handle(&self, spec: &Spec) -> Result<bool>;
}
pub struct Executor {
    handlers: Vec<Box<dyn ExecHandler>>,
}

impl Executor {
    pub fn new() -> Self {
        let mut handlers: Vec<Box<dyn ExecHandler>> = Vec::new();
        #[cfg(feature = "wasm-wasmer")]
        handlers.push(Box::new(WasmerExecHandler{}));
        handlers.push(Box::new(DefaultExecHandler{}));

        Self { handlers }
    }

    pub fn exec(&self, spec: &Spec) -> Result<()> {
        for handler in &self.handlers {
            if handler.can_handle(spec).with_context(|| format!("handler {} failed on selection",handler.name() ))? {
                handler.exec(spec).with_context(||format!("handler {} failed on exec", handler.name()))?;
            }
        }

        unreachable!("no suitable execution handler has been registered");
    }
}
