use crate::utils::{
    create_temp_dir, delete_container, generate_uuid, get_runtime_path, get_state, kill_container,
    prepare_bundle, State, TempDir,
};
use anyhow::anyhow;
use std::process::{Command, Stdio};
use test_framework::{Test, TestGroup, TestResult};
use uuid::Uuid;

#[inline]
fn cleanup(id: &Uuid, bundle: &TempDir) {
    kill_container(id, bundle).unwrap().wait().unwrap();
    delete_container(id, bundle).unwrap().wait().unwrap();
}

// here we have to manually create and manage the container
// as the test_inside container does not provide a way to set the pid file argument
fn test_pidfile() -> TestResult {
    // create id for the container and pidfile
    let container_id = generate_uuid();
    let pidfile_uuid = generate_uuid();

    // create temp dir for bundle and for storing the pid
    let bundle = prepare_bundle(&container_id).unwrap();
    let pidfile_dir = create_temp_dir(&pidfile_uuid).unwrap();

    // start the container
    Command::new(get_runtime_path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--root")
        .arg(bundle.as_ref().join("runtime"))
        .arg("create")
        .arg(container_id.to_string())
        .arg("--bundle")
        .arg(bundle.as_ref().join("bundle"))
        .arg("--pid-file")
        .arg(pidfile_dir.as_ref().join("pidfile"))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let (out, err) = get_state(&container_id, &bundle).unwrap();

    if !err.is_empty() {
        cleanup(&container_id, &bundle);
        return TestResult::Failed(anyhow!("error in state : {}", err));
    }

    let state: State = serde_json::from_str(&out).unwrap();

    if state.id != container_id.to_string() {
        cleanup(&container_id, &bundle);
        return TestResult::Failed(anyhow!(
            "error in state : id not matched ,expected {} got {}",
            container_id,
            state.id
        ));
    }

    if state.status != "created" {
        cleanup(&container_id, &bundle);
        return TestResult::Failed(anyhow!(
            "error in state : status not matched ,expected 'created' got {}",
            state.status
        ));
    }

    // get pid from the pidfile
    let pidfile: i32 = std::fs::read_to_string(pidfile_dir.as_ref().join("pidfile"))
        .unwrap()
        .parse()
        .unwrap();

    // get pid from the state
    if state.pid.unwrap() != pidfile {
        cleanup(&container_id, &bundle);
        return TestResult::Failed(anyhow!(
            "error : pid not matched ,expected {} as per state, but got {} from pidfile instead",
            state.pid.unwrap(),
            pidfile
        ));
    }

    cleanup(&container_id, &bundle);
    TestResult::Passed
}

pub fn get_pidfile_test<'a>() -> TestGroup<'a> {
    let pidfile = Test::new("pidfile", Box::new(test_pidfile));
    let mut tg = TestGroup::new("pidfile");
    tg.add(vec![Box::new(pidfile)]);
    tg
}
