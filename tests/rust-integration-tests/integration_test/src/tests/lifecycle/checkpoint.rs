use super::get_result_from_output;
use crate::utils::get_runtime_path;
use crate::utils::test_utils::State;
use crate::utils::{create_temp_dir, generate_uuid};
use anyhow::anyhow;
use std::path::Path;
use std::process::{Command, Stdio};
use test_framework::testable::TestError;
use test_framework::TestResult;

// Simple function to figure out the PID of the first container process
fn get_container_pid(project_path: &Path, id: &str) -> TestResult<i32> {
    let res_state = match Command::new(get_runtime_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("runtime"))
        .arg("state")
        .arg(id)
        .spawn()
        .expect("failed to execute state command")
        .wait_with_output()
    {
        Ok(o) => o,
        Err(e) => {
            return Err(TestError::Failed(anyhow!(
                "error getting container state {}",
                e
            )))
        }
    };
    let stdout = match String::from_utf8(res_state.stdout) {
        Ok(s) => s,
        Err(e) => {
            return Err(TestError::Failed(anyhow!(
                "failed to parse container stdout {}",
                e
            )))
        }
    };
    let state: State = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            return Err(TestError::Failed(anyhow!(
                "error in parsing state of container: stdout : {}, parse error : {}",
                stdout,
                e
            )))
        }
    };

    Ok(match state.pid {
        Some(p) => p,
        _ => -1,
    })
}

// CRIU requires a minimal network setup in the network namespace
fn setup_network_namespace(project_path: &Path, id: &str) -> TestResult<()> {
    let pid = get_container_pid(project_path, id)?;

    if let Err(e) = Command::new("nsenter")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-t")
        .arg(format!("{pid}"))
        .arg("-a")
        .args(vec!["/bin/ip", "link", "set", "up", "dev", "lo"])
        .spawn()
        .expect("failed to exec ip")
        .wait_with_output()
    {
        return Err(TestError::Failed(anyhow!(
            "error setting up network namespace {}",
            e
        )));
    }

    Ok(())
}

fn checkpoint(
    project_path: &Path,
    id: &str,
    args: Vec<&str>,
    work_path: Option<&str>,
) -> TestResult<()> {
    setup_network_namespace(project_path, id)?;

    let temp_dir = match create_temp_dir(&generate_uuid()) {
        Ok(td) => td,
        Err(e) => {
            return Err(TestError::Failed(anyhow::anyhow!(
                "failed creating temporary directory {:?}",
                e
            )))
        }
    };
    let checkpoint_dir = temp_dir.as_ref().join("checkpoint");
    if let Err(e) = std::fs::create_dir(&checkpoint_dir) {
        return Err(TestError::Failed(anyhow::anyhow!(
            "failed creating checkpoint directory ({:?}): {}",
            &checkpoint_dir,
            e
        )));
    }

    let additional_args = match work_path {
        Some(wp) => vec!["--work-path", wp],
        _ => Vec::new(),
    };

    let runtime_path = get_runtime_path();

    let checkpoint = Command::new(runtime_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--root")
        .arg(project_path.join("runtime"))
        .arg(match runtime_path {
            _ if runtime_path.ends_with("youki") => "checkpointt",
            _ => "checkpoint",
        })
        .arg("--image-path")
        .arg(&checkpoint_dir)
        .args(additional_args)
        .args(args)
        .arg(id)
        .spawn()
        .expect("failed to execute checkpoint command")
        .wait_with_output();

    get_result_from_output(checkpoint)?;

    // Check for complete checkpoint
    if !Path::new(&checkpoint_dir.join("inventory.img")).exists() {
        return Err(TestError::Failed(anyhow::anyhow!(
            "resulting checkpoint does not seem to be complete. {:?}/inventory.img is missing",
            &checkpoint_dir,
        )));
    }

    if !Path::new(&checkpoint_dir.join("descriptors.json")).exists() {
        return Err(TestError::Failed(anyhow::anyhow!(
            "resulting checkpoint does not seem to be complete. {:?}/descriptors.json is missing",
            &checkpoint_dir,
        )));
    }

    let dump_log = match work_path {
        Some(wp) => Path::new(wp).join("dump.log"),
        _ => checkpoint_dir.join("dump.log"),
    };

    if !dump_log.exists() {
        return Err(TestError::Failed(anyhow::anyhow!(
            "resulting checkpoint log file {:?} not found.",
            &dump_log,
        )));
    }

    Ok(())
}

pub fn checkpoint_leave_running_work_path_tmp(project_path: &Path, id: &str) -> TestResult<()> {
    checkpoint(project_path, id, vec!["--leave-running"], Some("/tmp/"))
}

pub fn checkpoint_leave_running(project_path: &Path, id: &str) -> TestResult<()> {
    checkpoint(project_path, id, vec!["--leave-running"], None)
}
