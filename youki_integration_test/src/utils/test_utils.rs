///! Contains utility functions for testing
///! Similar to https://github.com/opencontainers/runtime-tools/blob/master/validation/util/test.go
use super::get_runtime_path;
use super::{generate_uuid, prepare_bundle, set_config};
use anyhow::Result;
use oci_spec::runtime::Spec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::Duration;
use test_framework::TestResult;
use uuid::Uuid;

const SLEEP_TIME: Duration = Duration::from_millis(150);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub oci_version: String,
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    pub bundle: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<u32>,
    pub use_systemd: Option<bool>,
}

#[derive(Debug)]
pub struct ContainerData {
    pub id: String,
    pub state: Option<State>,
    pub state_err: String,
    pub exit_status: std::io::Result<ExitStatus>,
}

/// Starts the runtime with given directory as root directory
pub fn create_container<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<Child> {
    let res = Command::new(get_runtime_path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--root")
        .arg(dir.as_ref().join("runtime"))
        .arg("create")
        .arg(id.to_string())
        .arg("--bundle")
        .arg(dir.as_ref().join("bundle"))
        .spawn()?;
    Ok(res)
}

/// Sends a kill command to the given container process
pub fn kill_container<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<Child> {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(dir.as_ref().join("runtime"))
        .arg("kill")
        .arg(id.to_string())
        .arg("9")
        .spawn()?;
    Ok(res)
}

pub fn delete_container<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<Child> {
    let res = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(dir.as_ref().join("runtime"))
        .arg("delete")
        .arg(id.to_string())
        .spawn()?;
    Ok(res)
}

pub fn get_state<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<(String, String)> {
    sleep(SLEEP_TIME);
    let output = Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(dir.as_ref().join("runtime"))
        .arg("state")
        .arg(id.to_string())
        .spawn()?
        .wait_with_output()?;
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    Ok((stdout, stderr))
}

pub fn test_outside_container(spec: Spec, f: &dyn Fn(ContainerData) -> TestResult) -> TestResult {
    let id = generate_uuid();
    let bundle = prepare_bundle(&id).unwrap();
    set_config(&bundle, &spec).unwrap();
    let r = create_container(&id, &bundle).unwrap().wait();
    let (out, err) = get_state(&id, &bundle).unwrap();
    let state: Option<State> = match serde_json::from_str(&out) {
        Ok(v) => Some(v),
        Err(_) => None,
    };
    let data = ContainerData {
        id: id.to_string(),
        state,
        state_err: err,
        exit_status: r,
    };
    let ret = f(data);
    kill_container(&id, &bundle).unwrap().wait().unwrap();
    delete_container(&id, &bundle).unwrap().wait().unwrap();
    ret
}
