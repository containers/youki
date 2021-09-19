use once_cell::sync::OnceCell;
use std::env;
use std::path::Path;
use std::path::PathBuf;

static RUNTIME_PATH: OnceCell<PathBuf> = OnceCell::new();

pub fn set_runtime_path(path: &Path) {
    RUNTIME_PATH.set(path.to_owned()).unwrap();
}

pub fn get_runtime_path() -> &'static PathBuf {
    RUNTIME_PATH.get().expect("Runtime path is not set")
}

#[allow(dead_code)]
pub fn get_project_path() -> PathBuf {
    let current_dir_path_result = env::current_dir();
    match current_dir_path_result {
        Ok(path_buf) => path_buf,
        Err(e) => panic!("directory is not found, {}", e),
    }
}
