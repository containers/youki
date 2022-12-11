use anyhow::{bail, Context, Result};
use oci_spec::runtime::Spec;

pub mod default;

pub static EMPTY: Vec<String> = Vec::new();

pub trait Executor {
    /// Executes the workload
    fn exec(&self, spec: &Spec) -> Result<()>;

    /// Checks if the handler is able to handle the workload
    fn can_handle(&self, spec: &Spec) -> Result<bool>;

    /// The name of the handler
    fn name(&self) -> &'static str;
}

pub struct ExecutorManager {
    pub executors: Vec<Box<dyn Executor>>,
}

impl ExecutorManager {
    pub fn exec(&self, spec: &Spec) -> Result<()> {
        for executor in self.executors.iter() {
            if executor.can_handle(spec)? {
                return executor.exec(spec).context("execution failed");
            }
        }
        bail!("cannot find executor")
    }
}
