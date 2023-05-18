use oci_spec::runtime::Spec;
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    params, VmBuilder,
};

use libcontainer::workload::{Executor, ExecutorError};

const EXECUTOR_NAME: &str = "wasmedge";

#[derive(Default)]
pub struct WasmEdgeExecutor {}

impl WasmEdgeExecutor {
    fn exec_inner(spec: &Spec) -> anyhow::Result<()> {
        // parse wasi parameters
        let args = get_args(spec);
        let mut cmd = args[0].clone();
        if let Some(stripped) = args[0].strip_prefix(std::path::MAIN_SEPARATOR) {
            cmd = stripped.to_string();
        }
        let envs = env_to_wasi(spec);

        // create configuration with `wasi` option enabled
        let config = ConfigBuilder::new(CommonConfigOptions::default())
            .with_host_registration_config(HostRegistrationConfigOptions::default().wasi(true))
            .build()?;

        // create a vm with the config settings
        let mut vm = VmBuilder::new()
            .with_config(config)
            .build()?
            .register_module_from_file("main", cmd)?;

        // initialize the wasi module with the parsed parameters
        let wasi_instance = vm
            .wasi_module_mut()
            .expect("config doesn't contain HostRegistrationConfigOptions");
        wasi_instance.initialize(
            Some(args.iter().map(|s| s as &str).collect()),
            Some(envs.iter().map(|s| s as &str).collect()),
            None,
        );

        vm.run_func(Some("main"), "_start", params!())?;

        Ok(())
    }
}

impl Executor for WasmEdgeExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        Self::exec_inner(spec).map_err(|err| {
            tracing::error!(?err, "failed to execute workload with wasmedge handler");
            ExecutorError::Execution(err.into())
        })
    }

    fn can_handle(&self, spec: &Spec) -> bool {
        if let Some(annotations) = spec.annotations() {
            if let Some(handler) = annotations.get("run.oci.handler") {
                return handler == "wasm";
            }

            if let Some(variant) = annotations.get("module.wasm.image/variant") {
                return variant == "compat";
            }
        }

        false
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}

fn get_args(spec: &Spec) -> &[String] {
    let p = match spec.process() {
        None => return &[],
        Some(p) => p,
    };

    match p.args() {
        None => &[],
        Some(args) => args.as_slice(),
    }
}

fn env_to_wasi(spec: &Spec) -> Vec<String> {
    let default = vec![];
    let env = spec
        .process()
        .as_ref()
        .unwrap()
        .env()
        .as_ref()
        .unwrap_or(&default);
    env.to_vec()
}
