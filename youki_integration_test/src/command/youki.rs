use std::path::PathBuf;

use crate::support::generate_uuid;

use super::{create, delete, kill, start, state};

pub struct Container {
    project_path: PathBuf,
    container_id: String,
}

impl Container {
    pub fn new(project_path: &PathBuf) -> Self {
        Container {
            project_path: project_path.to_owned(),
            container_id: generate_uuid().to_string(),
        }
    }
    pub fn with_container_id(project_path: &PathBuf, container_id: &str) -> Self {
        Container {
            project_path: project_path.to_owned(),
            container_id: container_id.to_string(),
        }
    }

    pub fn create(&self) -> bool {
        create::exec(&self.project_path, &self.container_id)
    }

    pub fn start(&self) -> bool {
        start::exec(&self.project_path, &self.container_id)
    }

    pub fn state(&self) -> bool {
        state::exec(&self.project_path, &self.container_id)
    }

    pub fn kill(&self) -> bool {
        kill::exec(&self.project_path, &self.container_id)
    }

    pub fn delete(&self) -> bool {
        delete::exec(&self.project_path, &self.container_id)
    }
}
