use anyhow::anyhow;
use test_framework::{Test, TestGroup, TestResult};

use crate::tests::lifecycle::ContainerLifecycle;

fn kill_with_empty_id_test() -> TestResult {
    let mut container = ContainerLifecycle::new();

    // kill with empty id
    container.set_id("");
    let result = match container.kill() {
        TestResult::Failed(_) => TestResult::Passed,
        TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
        _ => TestResult::Failed(anyhow!("Unexpected test result")),
    };
    container.delete();
    result
}

fn kill_non_existed_container() -> TestResult {
    let mut container = ContainerLifecycle::new();

    // kill for non existed container
    container.set_id("non-existent-container-id");
    let result = match container.kill() {
        TestResult::Failed(_) => TestResult::Passed,
        TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
        _ => TestResult::Failed(anyhow!("Unexpected test result")),
    };
    container.delete();
    result
}
fn kill_created_container_test() -> TestResult {
    let container = ContainerLifecycle::new();

    // kill created container
    match container.create() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to create container")),
    }
    let result = match container.kill() {
        TestResult::Passed => TestResult::Passed,
        TestResult::Failed(_) => {
            TestResult::Failed(anyhow!("Expected success but got failure"))
        }
        _ => TestResult::Failed(anyhow!("Unexpected test result")),
    };
    container.delete();
    result
}

fn kill_stopped_container_test() -> TestResult {
    let container = ContainerLifecycle::new();

    // kill stopped container
    match container.create() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to create container")),
    }
    match container.delete() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to delete container")),
    }
    match container.kill() {
        TestResult::Failed(_) => TestResult::Passed,
        TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
        _ => TestResult::Failed(anyhow!("Unexpected test result")),
    }
}
    

fn kill_start_container_test() -> TestResult {
    let container = ContainerLifecycle::new();

    // kill start container
    match container.create() {
    TestResult::Passed => {}
    _ => return TestResult::Failed(anyhow!("Failed to recreate container")),
    }

    match container.start() {
        TestResult::Passed => {}
        TestResult::Failed(err) => {
            return TestResult::Failed(anyhow!("Failed to start container: {:?}", err));
        }
        _ => unreachable!(),
    }
    let result = match container.kill() {
        TestResult::Passed => TestResult::Passed,
        TestResult::Failed(_) => {
            TestResult::Failed(anyhow!("Expected success but got failure"))
        }
        _ => TestResult::Failed(anyhow!("Unexpected test result")),
    };
    container.delete();
    result
}


pub fn get_kill_test() -> TestGroup {
    let mut test_group = TestGroup::new("kill_container");

    let kill_with_empty_id_test = Test::new(
        "kill_with_empty_id_test",
        Box::new(kill_with_empty_id_test),
    );
    let kill_non_existed_container = Test::new(
        "kill_non_existed_container",
        Box::new(kill_non_existed_container)
    );
    let kill_created_container_test = Test::new(
        "kill_created_container_test",
        Box::new(kill_created_container_test)
    );
    let kill_stopped_container_test = Test::new(
        "kill_stopped_container_test",
        Box::new(kill_stopped_container_test)
    );
    let kill_start_container_test = Test::new(
        "kill_start_container_test",
        Box::new(kill_start_container_test)
    );
    test_group.add(vec![
        Box::new(kill_with_empty_id_test),
        Box::new(kill_non_existed_container),
        Box::new(kill_created_container_test),
        Box::new(kill_stopped_container_test),
        Box::new(kill_start_container_test)
    ]);
    test_group
}
