///! Contains utility functions for testing
///! Similar to https://github.com/opencontainers/runtime-tools/blob/master/validation/util/test.go
use super::get_runtime_path;
use super::{create_temp_dir, TempDir};
use anyhow::Result;
use flate2::read::GzDecoder;
use oci_spec::runtime::Spec;
use rand::Rng;
use std::fs::File;
use std::process::{Child, Command, Stdio};
use std::{fs, path::Path};
use tar::Archive;
use uuid::Uuid;

use std::thread::sleep;
use std::time::Duration;
const SLEEP_TIME: Duration = Duration::from_millis(150);

/// This will generate the UUID needed when creating the container.
pub fn generate_uuid() -> Uuid {
    let mut rng = rand::thread_rng();
    const CHARSET: &[u8] = b"0123456789abcdefABCDEF";

    let rand_string: String = (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    match Uuid::parse_str(&rand_string) {
        Ok(uuid) => uuid,
        Err(e) => panic!("can not parse uuid, {}", e),
    }
}

/// Creates a bundle directory in a temp directory
pub fn prepare_bundle(id: &Uuid) -> Result<TempDir> {
    let temp_dir = create_temp_dir(id)?;
    let tar_file_name = "bundle.tar.gz";
    let tar_path = std::env::current_dir()?.join(tar_file_name);
    fs::copy(tar_path.clone(), (&temp_dir).join(tar_file_name))?;
    let tar_gz = File::open(tar_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(&temp_dir)?;
    Ok(temp_dir)
}

/// Sets the config.json file as per given spec
#[allow(dead_code)]
pub fn set_config<P: AsRef<Path>>(project_path: P, config: &Spec) -> Result<()> {
    let path = project_path.as_ref().join("bundle").join("config.json");
    config.save(path)?;
    Ok(())
}

/// Starts the runtime with given directory as root directory
#[allow(dead_code)]
pub fn start_runtime<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<Child> {
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
#[allow(dead_code)]
pub fn stop_runtime<P: AsRef<Path>>(id: &Uuid, dir: P) -> Result<Child> {
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

#[allow(dead_code)]
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
