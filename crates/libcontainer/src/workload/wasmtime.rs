use anyhow::{anyhow, bail, Context, Result};
use oci_spec::runtime::Spec;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

use super::{Executor, EMPTY};

const EXECUTOR_NAME: &str = "wasmtime";

pub struct WasmtimeExecutor {}

impl Executor for WasmtimeExecutor {
    fn exec(spec: &Spec) -> Result<()> {
        log::info!("Executing workload with wasmtime handler");
        let process = spec.process().as_ref();

        let args = spec
            .process()
            .as_ref()
            .and_then(|p| p.args().as_ref())
            .unwrap_or(&EMPTY);
        if args.is_empty() {
            bail!("at least one process arg must be specified")
        }

        if !args[0].ends_with(".wasm") && !args[0].ends_with(".wat") {
            bail!(
                "first argument must be a wasm or wat module, but was {}",
                args[0]
            )
        }

        let mut cmd = args[0].clone();
        let stripped = args[0].strip_prefix(std::path::MAIN_SEPARATOR);
        if let Some(cmd_stripped) = stripped {
            cmd = cmd_stripped.to_string();
        }

        let envs: Vec<(String, String)> = process
            .and_then(|p| p.env().as_ref())
            .unwrap_or(&EMPTY)
            .iter()
            .filter_map(|e| {
                e.split_once('=')
                    .map(|kv| (kv.0.trim().to_string(), kv.1.trim().to_string()))
            })
            .collect();

        let engine = Engine::default();
        let module = Module::from_file(&engine, &cmd)
            .with_context(|| format!("could not load wasm module from {}", &cmd))?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .context("cannot add wasi context to linker")?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .args(args)
            .context("cannot add args to wasi context")?
            .envs(&envs)
            .context("cannot add environment variables to wasi context")?
            .build();

        let mut store = Store::new(&engine, wasi);

        let instance = linker
            .instantiate(&mut store, &module)
            .context("wasm module could not be instantiated")?;
        let start = instance
            .get_func(&mut store, "_start")
            .ok_or_else(|| anyhow!("could not retrieve wasm module main function"))?;

        start
            .call(&mut store, &[], &mut [])
            .context("wasm module was not executed successfully")
    }

    fn can_handle(spec: &Spec) -> Result<bool> {
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

    fn name() -> &'static str {
        EXECUTOR_NAME
    }
}
