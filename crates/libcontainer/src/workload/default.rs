use std::{
    ffi::CString,
    path::{Path, PathBuf},
};

use nix::unistd;
use oci_spec::runtime::Spec;

use super::{Executor, ExecutorError, EMPTY};

#[derive(Clone)]
pub struct DefaultExecutor {}

impl Executor for DefaultExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        tracing::debug!("executing workload with default handler");
        let args = spec
            .process()
            .as_ref()
            .and_then(|p| p.args().as_ref())
            .ok_or_else(|| {
                tracing::error!("no args provided to execute");
                ExecutorError::InvalidArg
            })?;
        let envs = spec
            .process()
            .as_ref()
            .and_then(|p| p.env().as_ref())
            .unwrap_or(&EMPTY);

        verify_binary(args, envs)?;

        let executable = args[0].as_str();
        let cstring_path = CString::new(executable.as_bytes()).map_err(|err| {
            tracing::error!("failed to convert path {executable:?} to cstring: {}", err,);
            ExecutorError::InvalidArg
        })?;
        let a: Vec<CString> = args
            .iter()
            .map(|s| CString::new(s.as_bytes()).unwrap_or_default())
            .collect();
        unistd::execvp(&cstring_path, &a).map_err(|err| {
            tracing::error!(?err, filename = ?cstring_path, args = ?a, "failed to execvp");
            ExecutorError::Execution(err.into())
        })?;

        // After execvp is called, the process is replaced with the container
        // payload through execvp, so it should never reach here.
        unreachable!();
    }
}

pub fn get_executor() -> Box<dyn Executor> {
    Box::new(DefaultExecutor {})
}

// this checks if the binary to run actually exists and if we have
// permissions to run it.  Taken from
// https://github.com/opencontainers/runc/blob/25c9e888686773e7e06429133578038a9abc091d/libcontainer/standard_init_linux.go#L195-L206
fn verify_binary(args: &[String], envs: &[String]) -> Result<(), ExecutorError> {
    let path_vars: Vec<&String> = envs.iter().filter(|&e| e.starts_with("PATH=")).collect();
    if path_vars.is_empty() {
        tracing::error!("PATH environment variable is not set");
        return Err(ExecutorError::InvalidArg);
    }
    let path_var = path_vars[0].trim_start_matches("PATH=");
    match get_executable_path(&args[0], path_var) {
        None => {
            tracing::error!(
                "executable {} for container process not found in PATH",
                args[0]
            );
            return Err(ExecutorError::InvalidArg);
        }
        Some(path) => match is_executable(&path) {
            Ok(true) => {
                tracing::debug!("found executable {:?}", path);
            }
            Ok(false) => {
                tracing::error!(
                    "executable {:?} does not have the correct permission set",
                    path
                );
                return Err(ExecutorError::InvalidArg);
            }
            Err(err) => {
                tracing::error!(
                    "failed to check permissions for executable {:?}: {}",
                    path,
                    err
                );
                return Err(ExecutorError::Other(format!(
                    "failed to check permissions for executable {:?}: {}",
                    path, err
                )));
            }
        },
    }
    Ok(())
}

fn get_executable_path(name: &str, path_var: &str) -> Option<PathBuf> {
    // if path has / in it, we have to assume absolute path, as per runc impl
    if name.contains('/') && PathBuf::from(name).exists() {
        return Some(PathBuf::from(name));
    }
    for path in path_var.split(':') {
        let potential_path = PathBuf::from(path).join(name);
        if potential_path.exists() {
            return Some(potential_path);
        }
    }
    None
}

fn is_executable(path: &Path) -> std::result::Result<bool, std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = path.metadata()?;
    let permissions = metadata.permissions();
    // we have to check if the path is file and the execute bit
    // is set. In case of directories, the execute bit is also set,
    // so have to check if this is a file or not
    Ok(metadata.is_file() && permissions.mode() & 0o001 != 0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_get_executable_path() {
        let non_existing_abs_path = "/some/non/existent/absolute/path";
        let existing_abs_path = "/usr/bin/sh";
        let existing_binary = "sh";
        let non_existing_binary = "non-existent";
        let path_value = "/usr/bin:/bin";

        assert_eq!(
            get_executable_path(existing_abs_path, path_value),
            Some(PathBuf::from(existing_abs_path))
        );
        assert_eq!(get_executable_path(non_existing_abs_path, path_value), None);

        assert_eq!(
            get_executable_path(existing_binary, path_value),
            Some(PathBuf::from("/usr/bin/sh"))
        );

        assert_eq!(get_executable_path(non_existing_binary, path_value), None);
    }

    #[test]
    fn test_is_executable() {
        let tmp = tempfile::tempdir().expect("create temp directory for test");
        let executable_path = PathBuf::from("/bin/sh");
        let directory_path = tmp.path();
        let non_executable_path = directory_path.join("non_executable_file");
        let non_existent_path = PathBuf::from("/some/non/existent/path");

        std::fs::File::create(non_executable_path.as_path()).unwrap();

        assert!(is_executable(&non_existent_path).is_err());
        assert!(is_executable(&executable_path).unwrap());
        assert!(!is_executable(&non_executable_path).unwrap());
        assert!(!is_executable(directory_path).unwrap());
    }
}
