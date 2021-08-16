use std::path::{Path, PathBuf};

use crate::support::generate_uuid;
use test_framework::{TestResult, TestableGroup};

use super::{create, delete, kill, start, state};

pub struct ContainerLifecycle {
    project_path: PathBuf,
    container_id: String,
}

impl ContainerLifecycle {
    pub fn new(project_path: &Path) -> Self {
        ContainerLifecycle {
            project_path: project_path.to_owned(),
            container_id: generate_uuid().to_string(),
        }
    }
    pub fn with_container_id(project_path: &Path, container_id: &str) -> Self {
        ContainerLifecycle {
            project_path: project_path.to_owned(),
            container_id: container_id.to_string(),
        }
    }

    pub fn create(&self) -> TestResult {
        create::create(&self.project_path, &self.container_id)
    }

    pub fn start(&self) -> TestResult {
        start::start(&self.project_path, &self.container_id)
    }

    pub fn state(&self) -> TestResult {
        state::state(&self.project_path, &self.container_id)
    }

    pub fn kill(&self) -> TestResult {
        kill::kill(&self.project_path, &self.container_id)
    }

    pub fn delete(&self) -> TestResult {
        delete::delete(&self.project_path, &self.container_id)
    }
}

impl TestableGroup for ContainerLifecycle {
    fn get_name(&self) -> String {
        "lifecycle".to_owned()
    }
    fn run_all(&self) -> Vec<(String, TestResult)> {
        vec![
            ("create".to_owned(), self.create()),
            ("start".to_owned(), self.start()),
            ("kill".to_owned(), self.kill()),
            ("state".to_owned(), self.state()),
            ("delete".to_owned(), self.delete()),
        ]
    }
    fn run_selected(&self, selected: &[&str]) -> Vec<(String, TestResult)> {
        let mut ret = Vec::new();
        for name in selected {
            match *name {
                "create" => ret.push(("create".to_owned(), self.create())),
                "start" => ret.push(("start".to_owned(), self.start())),
                "kill" => ret.push(("kill".to_owned(), self.kill())),
                "state" => ret.push(("state".to_owned(), self.state())),
                "delete" => ret.push(("delete".to_owned(), self.delete())),
                _ => eprintln!("No test named {} in lifecycle", name),
            };
        }
        ret
    }
}
