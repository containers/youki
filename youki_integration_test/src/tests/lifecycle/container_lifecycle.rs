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

impl<'a> TestableGroup<'a> for ContainerLifecycle {
    fn get_name(&self) -> &'a str {
        "lifecycle"
    }
    fn run_all(&self) -> Vec<(&'a str, TestResult)> {
        vec![
            ("create", self.create()),
            ("start", self.start()),
            ("kill", self.kill()),
            ("state", self.state()),
            ("delete", self.delete()),
        ]
    }
    fn run_selected(&self, selected: &[&str]) -> Vec<(&'a str, TestResult)> {
        let mut ret = Vec::new();
        for name in selected {
            match *name {
                "create" => ret.push(("create", self.create())),
                "start" => ret.push(("start", self.start())),
                "kill" => ret.push(("kill", self.kill())),
                "state" => ret.push(("state", self.state())),
                "delete" => ret.push(("delete", self.delete())),
                _ => eprintln!("No test named {} in lifecycle", name),
            };
        }
        ret
    }
}
