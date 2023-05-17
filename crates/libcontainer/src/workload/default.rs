use std::ffi::CString;

use nix::unistd;
use oci_spec::runtime::Spec;

use super::{Executor, ExecutorError, EMPTY};

const EXECUTOR_NAME: &str = "default";

#[derive(Default)]
pub struct DefaultExecutor {}

impl Executor for DefaultExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        tracing::debug!("executing workload with default handler");
        let args = spec
            .process()
            .as_ref()
            .and_then(|p| p.args().as_ref())
            .unwrap_or(&EMPTY);

        if args.is_empty() {
            tracing::error!("no arguments provided to execute");
            Err(ExecutorError::InvalidArg)?;
        }

        let executable = args[0].as_str();
        let p = CString::new(executable.as_bytes()).map_err(|err| {
            tracing::error!("failed to convert path {executable:?} to cstring: {}", err,);
            ExecutorError::InvalidArg
        })?;
        let a: Vec<CString> = args
            .iter()
            .map(|s| CString::new(s.as_bytes()).unwrap_or_default())
            .collect();
        unistd::execvp(&p, &a).map_err(|err| {
            tracing::error!(?err, filename = ?p, args = ?a, "failed to execvp");
            ExecutorError::Execution(err.into())
        })?;

        // After do_exec is called, the process is replaced with the container
        // payload through execvp, so it should never reach here.
        unreachable!();
    }

    fn can_handle(&self, _: &Spec) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}
