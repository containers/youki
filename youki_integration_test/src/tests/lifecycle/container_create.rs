use super::create;
use crate::support::generate_uuid;
use std::path::{Path, PathBuf};
use test_framework::{TestResult, TestableGroup};

pub struct ContainerCreate {
    project_path: PathBuf,
    container_id: String,
}

impl ContainerCreate {
    pub fn new(project_path: &Path) -> Self {
        ContainerCreate {
            project_path: project_path.to_owned(),
            container_id: generate_uuid().to_string(),
        }
    }

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

    fn create_valid_id(&self) -> TestResult {
        create::create(&self.project_path, &self.container_id)
    }
    fn create_duplicate_id(&self) -> TestResult {
        let id = generate_uuid().to_string();
        let _ = create::create(&self.project_path, &id);
        let temp = create::create(&self.project_path, &id);
        match temp {
            TestResult::Ok => TestResult::Err(anyhow::anyhow!(
                "Container should not have been created with same id, but was created."
            )),
            TestResult::Err(_) => TestResult::Ok,
            TestResult::Skip => TestResult::Skip,
        }
    }
}

impl TestableGroup for ContainerCreate {
    fn get_name(&self) -> String {
        "create".to_owned()
    }
    fn run_all(&self) -> Vec<(String, TestResult)> {
        vec![
            ("empty_id".to_owned(), self.create_empty_id()),
            ("valid_id".to_owned(), self.create_valid_id()),
            ("duplicate_id".to_owned(), self.create_duplicate_id()),
        ]
    }
    fn run_selected(&self, selected: &[&str]) -> Vec<(String, TestResult)> {
        let mut ret = Vec::new();
        for name in selected {
            match *name {
                "empty_id" => ret.push(("empty_id".to_owned(), self.create_empty_id())),
                "valid_id" => ret.push(("valid_id".to_owned(), self.create_valid_id())),
                "duplicate_id" => ret.push(("duplicate_id".to_owned(), self.create_duplicate_id())),
                _ => eprintln!("No test named {} in lifecycle", name),
            };
        }
        ret
    }
}
