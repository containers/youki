use std::path::PathBuf;

use anyhow::Result;
use oci_spec::runtime::Spec;
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    params, Vm,
};

use super::Executor;

const EXECUTOR_NAME: &str = "wasmedge";

pub struct WasmEdge {}

fn get_root(spec: &Spec) -> &PathBuf {
    let root = spec.root().as_ref().unwrap();
    root.path()
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

impl Executor for WasmEdge {
    fn exec(spec: &Spec) -> Result<()> {
        let host_options = HostRegistrationConfigOptions::default().wasi(true);
        let config = ConfigBuilder::new(CommonConfigOptions::default())
            .with_host_registration_config(host_options)
            .build()?;
        let mut vm = Vm::new(Some(config))?;

        let args = get_args(&spec);
        let mut cmd = args[0].clone();
        if let Some(stripped) = args[0].strip_prefix(std::path::MAIN_SEPARATOR) {
            cmd = stripped.to_string();
        }

        let envs = env_to_wasi(&spec);

        let mut wasi_instance = vm.wasi_module()?;
        wasi_instance.initialize(
            Some(args.iter().map(|s| s as &str).collect()),
            Some(envs.iter().map(|s| s as &str).collect()),
            Some(vec![&cmd]),
        );

        let vm = vm.register_module_from_file("main", &cmd)?;

        vm.run_func(Some("main"), "_start", params!())?;

        Ok(())
    }

    fn can_handle(_: &Spec) -> Result<bool> {
        Ok(true)
    }

    fn name() -> &'static str {
        EXECUTOR_NAME
    }
}
