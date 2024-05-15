use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use wasmedge_sdk::config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions};
use wasmedge_sdk::{params, VmBuilder};

const EXECUTOR_NAME: &str = "wasmedge";

#[derive(Clone)]
pub struct WasmedgeExecutor {}

impl Executor for WasmedgeExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }

        tracing::debug!("executing workload with wasmedge handler");

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
            .build()
            .map_err(|err| {
                ExecutorError::Other(format!("failed to create wasmedge config: {}", err))
            })?;

        // create a vm with the config settings
        let mut vm = VmBuilder::new()
            .with_config(config)
            .build()
            .map_err(|err| ExecutorError::Other(format!("failed to create wasmedge vm: {}", err)))?
            .register_module_from_file("main", cmd)
            .map_err(|err| {
                ExecutorError::Other(format!(
                    "failed to register wasmedge module from the file: {}",
                    err
                ))
            })?;
        // initialize the wasi module with the parsed parameters
        let wasi_instance = vm
            .wasi_module_mut()
            .expect("config doesn't contain HostRegistrationConfigOptions");
        wasi_instance.initialize(
            Some(args.iter().map(|s| s as &str).collect()),
            Some(envs.iter().map(|s| s as &str).collect()),
            None,
        );

        vm.run_func(Some("main"), "_start", params!())
            .map_err(|err| ExecutorError::Execution(err))?;

        Ok(())
    }

    fn validate(&self, spec: &Spec) -> Result<(), ExecutorValidationError> {
        if !can_handle(spec) {
            return Err(ExecutorValidationError::CantHandle(EXECUTOR_NAME));
        }

        Ok(())
    }
}

pub fn get_executor() -> WasmedgeExecutor {
    WasmedgeExecutor {}
}

fn can_handle(spec: &Spec) -> bool {
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
    // below we can be sure that process exists, as otherwise container init process
    // function would have returned error at the very start
    let env = spec
        .process()
        .as_ref()
        .unwrap()
        .env()
        .as_ref()
        .unwrap_or(&default);
    env.to_vec()
}
