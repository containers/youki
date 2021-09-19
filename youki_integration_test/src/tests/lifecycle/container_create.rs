use super::{create, kill};
use crate::support::generate_uuid;
use std::path::{Path, PathBuf};
use test_framework::{TestResult, TestableGroup};

pub struct ContainerCreate {
    project_path: PathBuf,
    container_id: String,
}

impl<'a> ContainerCreate {
    pub fn new(project_path: &Path) -> Self {
        ContainerCreate {
            project_path: project_path.to_owned(),
            container_id: generate_uuid().to_string(),
        }
    }

    // runtime should not create container with empty id
    fn create_empty_id(&self) -> TestResult {
        let temp = create::create(&self.project_path, "");
        match temp {
            TestResult::Ok => TestResult::Err(anyhow::anyhow!(
                "Container should not have been created with empty id, but was created."
            )),
            TestResult::Err(_) => TestResult::Ok,
            TestResult::Skip => TestResult::Skip,
        }
    }

    // runtime should create container with valid id
    fn create_valid_id(&self) -> TestResult {
        let temp = create::create(&self.project_path, &self.container_id);
        if let TestResult::Ok = temp {
            kill::kill(&self.project_path, &self.container_id);
        }
        temp
    }

    // runtime should not create container with is that already exists
    fn create_duplicate_id(&self) -> TestResult {
        let id = generate_uuid().to_string();
        let _ = create::create(&self.project_path, &id);
        let temp = create::create(&self.project_path, &id);
        kill::kill(&self.project_path, &id);
        match temp {
            TestResult::Ok => TestResult::Err(anyhow::anyhow!(
                "Container should not have been created with same id, but was created."
            )),
            TestResult::Err(_) => TestResult::Ok,
            TestResult::Skip => TestResult::Skip,
        }
    }
}

impl<'a> TestableGroup<'a> for ContainerCreate {
    fn get_name(&self) -> &'a str {
        "create"
    }

    fn run_all(&self) -> Vec<(&'a str, TestResult)> {
        vec![
            ("empty_id", self.create_empty_id()),
            ("valid_id", self.create_valid_id()),
            ("duplicate_id", self.create_duplicate_id()),
        ]
    }

    fn run_selected(&self, selected: &[&str]) -> Vec<(&'a str, TestResult)> {
        let mut ret = Vec::new();
        for name in selected {
            match *name {
                "empty_id" => ret.push(("empty_id", self.create_empty_id())),
                "valid_id" => ret.push(("valid_id", self.create_valid_id())),
                "duplicate_id" => ret.push(("duplicate_id", self.create_duplicate_id())),
                _ => eprintln!("No test named {} in lifecycle", name),
            };
        }
        ret
    }
}
