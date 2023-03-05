use crate::utils::{self, test_read_access, test_write_access};
use anyhow::{bail, Result};
use nix::errno::Errno;
use oci_spec::runtime::Spec;
use std::fs::read_dir;
use std::path::Path;

pub fn validate_readonly_paths(spec: &Spec) {
    let linux = spec.linux().as_ref().unwrap();
    let ro_paths = match linux.readonly_paths() {
        Some(p) => p,
        None => {
            eprintln!("in readonly paths, expected some readonly paths to be set, found none");
            return;
        }
    };

    if ro_paths.is_empty() {
        return;
    }

    // TODO when https://github.com/rust-lang/rust/issues/86442 stabilizes,
    // change manual matching of i32 to e.kind() and match statement
    for path in ro_paths {
        if let std::io::Result::Err(e) = test_read_access(path) {
            let errno = Errno::from_i32(e.raw_os_error().unwrap());
            // In the integration tests we test for both existing and non-existing readonly paths
            // to be specified in the spec, so we allow ENOENT here
            if errno == Errno::ENOENT {
                /* This is expected */
            } else {
                eprintln!(
                    "in readonly paths, error in testing read access for path {path} : {e:?}"
                );
                return;
            }
        } else {
            /* Expected */
        }

        if let std::io::Result::Err(e) = test_write_access(path) {
            let errno = Errno::from_i32(e.raw_os_error().unwrap());
            // In the integration tests we test for both existing and non-existing readonly paths
            // being specified in the spec, so we allow ENOENT, and we expect EROFS as the paths
            // should be read-only
            if errno == Errno::ENOENT || errno == Errno::EROFS {
                /* This is expected */
            } else {
                eprintln!(
                    "in readonly paths, error in testing write access for path {path} : {e:?}"
                );
                return;
            }
        } else {
            eprintln!("in readonly paths, path {path} expected to not be writable, found writable");
            return;
        }
    }
}

pub fn validate_hostname(spec: &Spec) {
    if let Some(expected_hostname) = spec.hostname() {
        if expected_hostname.is_empty() {
            // Skipping empty hostname
            return;
        }
        let actual_hostname = nix::unistd::gethostname().expect("failed to get current hostname");
        let actual_hostname = actual_hostname.to_str().unwrap();
        if actual_hostname != expected_hostname {
            eprintln!(
                "Unexpected hostname, expected: {expected_hostname:?} found: {actual_hostname:?}"
            );
        }
    }
}

// Run argument test recursively for files after base_dir
fn do_test_mounts_recursive(base_dir: &Path, test_fn: &dyn Fn(&Path) -> Result<()>) -> Result<()> {
    let dirs = read_dir(base_dir).unwrap();
    for dir in dirs {
        let dir = dir.unwrap();
        let f_type = dir.file_type().unwrap();
        if f_type.is_dir() {
            do_test_mounts_recursive(dir.path().as_path(), test_fn)?;
        }

        if f_type.is_file() {
            test_fn(dir.path().as_path())?;
        }
    }

    Ok(())
}

pub fn validate_mounts_recursive(spec: &Spec) {
    if let Some(mounts) = spec.mounts() {
        for mount in mounts {
            if let Some(options) = mount.options() {
                for option in options {
                    match option.as_str() {
                        "rro" => {
                            if let Err(e) =
                                do_test_mounts_recursive(mount.destination(), &|test_file_path| {
                                    if utils::test_write_access(test_file_path.to_str().unwrap())
                                        .is_ok()
                                    {
                                        // Return Err if writeable
                                        bail!(
                                            "path {:?} expected to be read-only, found writable",
                                            test_file_path
                                        );
                                    }
                                    Ok(())
                                })
                            {
                                eprintln!("error in testing rro recursive mounting : {e}");
                            }
                        }
                        "rnoexec" => {
                            if let Err(e) = do_test_mounts_recursive(
                                mount.destination(),
                                &|test_file_path| {
                                    if utils::test_file_executable(test_file_path.to_str().unwrap())
                                        .is_ok()
                                    {
                                        bail!("path {:?} expected to be not executable, found executable", test_file_path);
                                    }
                                    Ok(())
                                },
                            ) {
                                eprintln!("error in testing rnoexec recursive mounting: {e}");
                            }
                        }
                        "rdiratime" => {
                            println!("test_dir_update_access_time: {:?}", mount);
                            let rest = utils::test_dir_update_access_time(
                                mount.destination().to_str().unwrap(),
                            );
                            if let Err(e) = rest {
                                eprintln!("error in testing rdiratime recursive mounting: {e}");
                            }
                        }
                        "rnodiratime" => {
                            println!("test_dir_not_update_access_time: {:?}", mount);
                            let rest = utils::test_dir_not_update_access_time(
                                mount.destination().to_str().unwrap(),
                            );
                            if let Err(e) = rest {
                                eprintln!("error in testing rnodiratime recursive mounting: {e}");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
