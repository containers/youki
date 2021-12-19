use anyhow::{bail, Context, Result};
use oci_spec::runtime::Spec;
use wasmer::{Instance, Module, Store};
use wasmer_wasi::WasiState;

static EMPTY: Vec<String> = Vec::new();

pub fn exec(spec: &Spec) -> Result<()> {
    let process = spec.process().as_ref();

    let args = process
        .and_then(|p| p.args().as_ref())
        .unwrap_or_else(|| &EMPTY);
    let env = process
        .and_then(|p| p.env().as_ref())
        .unwrap_or_else(|| &EMPTY)
        .into_iter()
        .filter_map(|e| {
            e.split_once("=")
                .filter(|kv| !kv.0.contains('\u{0}') && !kv.1.contains('\u{0}'))
                .map(|kv| (kv.0.trim(), kv.1.trim()))
        });

    if args.len() == 0 {
        bail!("at least one process arg must be specified")
    }

    if !args[0].ends_with(".wasm") {
        bail!("first argument must be a wasm module, but was {}", args[0])
    }

    let mut wasm_env = WasiState::new("youki_wasm_app")
        .args(args.iter().skip(1))
        .envs(env)
        .finalize()?;

    let store = Store::default();
    let module = Module::from_file(&store, "hello.wasm").context("could not load wasm module")?;

    let imports = wasm_env
        .import_object(&module)
        .context("could not retrieve wasm imports")?;
    let instance =
        Instance::new(&module, &imports).context("wasm module could not be instantiated")?;

    let start = instance
        .exports
        .get_function("_start")
        .context("could not retrieve wasm module main function")?;
    start
        .call(&[])
        .context("wasm module was not executed successfuly")?;

    Ok(())
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
