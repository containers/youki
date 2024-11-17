use anyhow::anyhow;
use test_framework::{ConditionalTest, TestGroup, TestResult};

use crate::tests::lifecycle::ContainerLifecycle;

fn run_kill_test_cases() -> TestResult {
    let mut container = ContainerLifecycle::new();
    let mut results = vec![];
    let container_id = container.get_id().to_string();

    // kill with empty id
    container.set_id("");
    results.push((
        "kill without ID",
        match container.kill() {
            TestResult::Failed(_) => TestResult::Passed,
            TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
            _ => TestResult::Failed(anyhow!("Unexpected test result")),
        },
    ));

    // kill for non existed container
    container.set_id("non-existent-container-id");
    results.push((
        "kill non-existent container",
        match container.kill() {
            TestResult::Failed(_) => TestResult::Passed,
            TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
            _ => TestResult::Failed(anyhow!("Unexpected test result")),
        },
    ));

    // kill created container
    container.set_id(&container_id);
    match container.create() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to create container")),
    }
    results.push((
        "kill created container",
        match container.kill() {
            TestResult::Passed => TestResult::Passed,
            TestResult::Failed(_) => {
                TestResult::Failed(anyhow!("Expected success but got failure"))
            }
            _ => TestResult::Failed(anyhow!("Unexpected test result")),
        },
    ));

    // kill stopped container
    match container.delete() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to delete container")),
    }
    results.push((
        "kill stopped container",
        match container.kill() {
            TestResult::Failed(_) => TestResult::Passed,
            TestResult::Passed => TestResult::Failed(anyhow!("Expected failure but got success")),
            _ => TestResult::Failed(anyhow!("Unexpected test result")),
        },
    ));

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
    results.push((
        "kill running container",
        match container.kill() {
            TestResult::Passed => TestResult::Passed,
            TestResult::Failed(_) => {
                TestResult::Failed(anyhow!("Expected success but got failure"))
            }
            _ => TestResult::Failed(anyhow!("Unexpected test result")),
        },
    ));

    match container.delete() {
        TestResult::Passed => {}
        _ => return TestResult::Failed(anyhow!("Failed to delete container")),
    }

    for (name, result) in results {
        if let TestResult::Failed(err) = result {
            return TestResult::Failed(anyhow!("Test '{}' failed: {:?}", name, err));
        }
    }

    TestResult::Passed
}

pub fn get_kill_test() -> TestGroup {
    let mut test_group = TestGroup::new("kill_container");
    let kill_test = ConditionalTest::new(
        "test_kill_container",
        Box::new(|| true),
        Box::new(run_kill_test_cases),
    );
    test_group.add(vec![Box::new(kill_test)]);
    test_group
}
