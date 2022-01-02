use test_framework::{Test, TestGroup, TestResult};

// runtime should not create container with empty id
fn create_empty_id() -> TestResult {
    // let temp = create::create(&self.project_path, "");
    // match temp {
    //     TestResult::Passed => TestResult::Failed(anyhow::anyhow!(
    //         "Container should not have been created with empty id, but was created."
    //     )),
    //     TestResult::Failed(_) => TestResult::Passed,
    //     TestResult::Skipped => TestResult::Skipped,
    // }
    todo!()
}

// runtime should create container with valid id
fn create_valid_id(&self) -> TestResult {
    let temp = create::create(&self.project_path, &self.container_id);
    if let TestResult::Passed = temp {
        kill::kill(&self.project_path, &self.container_id);
        delete::delete(&self.project_path, &self.container_id);
    }
    temp
}

// runtime should not create container with is that already exists
fn create_duplicate_id(&self) -> TestResult {
    let id = generate_uuid().to_string();
    let _ = create::create(&self.project_path, &id);
    let temp = create::create(&self.project_path, &id);
    kill::kill(&self.project_path, &id);
    delete::delete(&self.project_path, &id);
    match temp {
        TestResult::Passed => TestResult::Failed(anyhow::anyhow!(
            "Container should not have been created with same id, but was created."
        )),
        TestResult::Failed(_) => TestResult::Passed,
        TestResult::Skipped => TestResult::Skipped,
    }
}

pub fn get_create_test_group<'a>() -> TestGroup<'a> {
    let empty_id = Box::new(Test::new("empty_id", Box::new(create_empty_id)));
    let valid_id = Box::new(Test::new("valid_id", Box::new(create_valid_id)));
    let duplicate_id = Box::new(Test::new("duplicate_id", Box::new(create_duplicate_id)));

    let mut tg = TestGroup::new("create");
    tg.add(vec![empty_id, valid_id, duplicate_id]);

    tg
}
