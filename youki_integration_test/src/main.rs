use anyhow::{bail, Result};

mod support;
mod tests;

use crate::support::cleanup_test;
use crate::support::get_project_path;
use crate::support::initialize_test;
use crate::tests::lifecycle::ContainerLifecycle;
use test_framework::TestManager;

fn main() -> Result<()> {
    let mut tm = TestManager::new();
    let project_path = get_project_path();
    let cl = ContainerLifecycle::new(&project_path);
    tm.add_test_group(&cl);
    if initialize_test(&project_path).is_err() {
        bail!("Can not initilize test.")
    }
    tm.run_all();
    //life_cycle_test(&project_path);
    if cleanup_test(&project_path).is_err() {
        bail!("Can not cleanup test.")
    }

    if initialize_test(&project_path).is_err() {
        bail!("Can not initilize test.")
    }
    //container_create_test(&project_path);
    if cleanup_test(&project_path).is_err() {
        bail!("Can not cleanup test.")
    }
    Ok(())
}

// This tests the entire lifecycle of the container.
// fn life_cycle_test(project_path: &Path) {
//     let container_runtime = Container::new(project_path);

//     let create_test = test_builder(
//         container_runtime.create(),
//         "Create a new container test",
//         "This operation must create a new container.",
//     );
//     let state_test = test_builder(
//         container_runtime.state(),
//         "Execute state test",
//         "This operation must state the container.",
//     );
//     let start_test = test_builder(
//         container_runtime.start(),
//         "Execute start test",
//         "This operation must start the container.",
//     );
//     let state_again_test = test_builder(
//         container_runtime.state(),
//         "Execute state test",
//         "This operation must state the container.",
//     );
//     let kill_test = test_builder(
//         container_runtime.kill(),
//         "Execute kill test",
//         "This operation must kill the container.",
//     );
//     let delete_test = test_builder(
//         container_runtime.delete(),
//         "Execute delete test",
//         "This operation must delete the container.",
//     );

//     // print to stdout
//     print_test_results(
//         "Create comand test suite",
//         vec![
//             create_test,
//             state_test,
//             start_test,
//             state_again_test,
//             kill_test,
//             delete_test,
//         ],
//     );
// }

// This is a test of the create command.
// It follows the `opencontainers/runtime-tools` test case.
// fn container_create_test(project_path: &Path) {
//     let container_runtime_with_empty_id = Container::with_container_id(project_path, "");
//     let empty_id_test = test_builder(
//         !container_runtime_with_empty_id.create(),
//         "create with no ID test",
//         "This operation MUST generate an error if it is not provided a path to the bundle and the container ID to associate with the container.",
//     );

//     let uuid = generate_uuid();
//     let container_runtime_with_id = Container::with_container_id(project_path, &uuid.to_string());
//     let with_id_test = test_builder(
//         container_runtime_with_id.create(),
//         "create with ID test",
//         "This operation MUST create a new container.",
//     );

//     let container_id_with_exist_id = Container::with_container_id(project_path, &uuid.to_string());
//     let exist_id_test = test_builder(
//         !container_id_with_exist_id.create(),
//         "create with an already existing ID test",
//         "If the ID provided is not unique across all containers within the scope of the runtime, or is not valid in any other way, the implementation MUST generate an error and a new container MUST NOT be created.",
//     );

//     // print to stdout
//     print_test_results(
//         "Create comand test suite",
//         vec![empty_id_test, with_id_test, exist_id_test],
//     );
// }
