#[cfg(test)]
mod integration {
    mod support;
    mod create;
    mod start;
    mod state;
    mod kill;
    mod delete;

    // This tests the entire lifecycle of the container.
    #[test]
    fn valid_life_cycle() {
        let project_path = support::create_project_path();
        let uuid = support::generate_uuid();
        if  support::initialize_test(&project_path).is_err() {
            panic!("Can not initilize test.");
        }

        if !create::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must create a new container.");
        }
        if !state::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must state the container.");
        }
        if !start::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must start the container.");
        }
        if !state::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must state the container.");
        }
        if !kill::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must kill the container.");
        }
        if !delete::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation must delete the container.");
        }

        if support::cleanup_test(&project_path).is_err() {
            panic!("Can not cleanup test.");
        }
    }

    // This is a test of the create command.
    // It follows the `opencontainers/runtime-tools` test case.
    #[test]
    fn create() {
        let project_path = support::create_project_path();
        let uuid = support::generate_uuid();
        if  support::initialize_test(&project_path).is_err() {
            panic!("Can not initilize test.");
        }

        if create::exec(&project_path, "") {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation MUST generate an error if it is not provided a path to the bundle and the container ID to associate with the container.");
        }
        if !create::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("This operation MUST create a new container.");
        }
        if create::exec(&project_path, &uuid.to_string()) {
            if support::cleanup_test(&project_path).is_err() {
                panic!("Can not cleanup test.");
            }
            panic!("If the ID provided is not unique across all containers within the scope of the runtime, or is not valid in any other way, the implementation MUST generate an error and a new container MUST NOT be created.");
        }

        if support::cleanup_test(&project_path).is_err() {
            panic!("Can not cleanup test.");
        }
    }
}
