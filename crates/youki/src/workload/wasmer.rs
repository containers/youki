use anyhow::{bail, Context, Result};
use oci_spec::runtime::Spec;
use wasmer::{Instance, Module, Store};
use wasmer_wasix::WasiEnv;

use libcontainer::workload::{Executor, EMPTY};

const EXECUTOR_NAME: &str = "wasmer";

#[derive(Default)]
pub struct WasmerExecutor {}

impl Executor for WasmerExecutor {
    fn exec(&self, spec: &Spec) -> Result<()> {
        log::debug!("Executing workload with wasmer handler");
        let process = spec.process().as_ref();

        let args = process.and_then(|p| p.args().as_ref()).unwrap_or(&EMPTY);
        let env = process
            .and_then(|p| p.env().as_ref())
            .unwrap_or(&EMPTY)
            .iter()
            .filter_map(|e| {
                e.split_once('=')
                    .filter(|kv| !kv.0.contains('\u{0}') && !kv.1.contains('\u{0}'))
                    .map(|kv| (kv.0.trim(), kv.1.trim()))
            });

        if args.is_empty() {
            bail!("at least one process arg must be specified")
        }

        if !args[0].ends_with(".wasm") && !args[0].ends_with(".wat") {
            bail!(
                "first argument must be a wasm or wat module, but was {}",
                args[0]
            )
        }


        let mut store = Store::default();
        let module = Module::from_file(&store, &args[0])
            .with_context(|| format!("could not load wasm module from {}", &args[0]))?;

        let mut wasi_env = WasiEnv::builder("youki_wasm_app")
            .args(args.iter().skip(1))
            .envs(env)
            .finalize(&mut store)?;

        let imports = wasi_env
            .import_object(&mut store, &module)
            .context("could not retrieve wasm imports")?;
        let instance =
            Instance::new(&mut store, &module, &imports).context("wasm module could not be instantiated")?;
        
        wasi_env.initialize(&mut store, instance.clone())?;

        let start = instance
            .exports
            .get_function("_start")
            .context("could not retrieve wasm module main function")?;
        start
            .call(&mut store, &[])
            .context("wasm module was not executed successfully")?;

        wasi_env.cleanup(&mut store, None);

        Ok(())
    }

    fn can_handle(&self, spec: &Spec) -> Result<bool> {
        if let Some(annotations) = spec.annotations() {
            if let Some(handler) = annotations.get("run.oci.handler") {
                return Ok(handler == "wasm");
            }

            if let Some(variant) = annotations.get("module.wasm.image/variant") {
                return Ok(variant == "compat");
            }
        }

        Ok(false)
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::runtime::SpecBuilder;
    use std::collections::HashMap;

    #[test]
    fn test_can_handle_oci_handler() -> Result<()> {
        let mut annotations = HashMap::with_capacity(1);
        annotations.insert("run.oci.handler".to_owned(), "wasm".to_owned());
        let spec = SpecBuilder::default()
            .annotations(annotations)
            .build()
            .context("build spec")?;

        assert!(WasmerExecutor::default()
            .can_handle(&spec)
            .context("can handle")?);

        Ok(())
    }

    #[test]
    fn test_can_handle_compat_wasm_spec() -> Result<()> {
        let mut annotations = HashMap::with_capacity(1);
        annotations.insert("module.wasm.image/variant".to_owned(), "compat".to_owned());
        let spec = SpecBuilder::default()
            .annotations(annotations)
            .build()
            .context("build spec")?;

        assert!(WasmerExecutor::default()
            .can_handle(&spec)
            .context("can handle")?);

        Ok(())
    }

    #[test]
    fn test_can_handle_no_execute() -> Result<()> {
        let spec = SpecBuilder::default().build().context("build spec")?;

        assert!(!WasmerExecutor::default()
            .can_handle(&spec)
            .context("can handle")?);

        Ok(())
    }
}
