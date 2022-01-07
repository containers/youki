use crate::utils::{test_read_access, test_write_access, AccessibilityStatus};
use oci_spec::runtime::Spec;

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
        eprintln!("in readonly paths, expected some readonly paths to be set, found none");
        return;
    }

    for path in ro_paths {
        match test_read_access(path) {
            std::io::Result::Err(e) => {
                eprintln!(
                    "in readonly paths, error in testing read access for path {} : {:?}",
                    path, e
                );
                return;
            }
            Ok(readability) => {
                match readability {
                    AccessibilityStatus::Accessible => { /* This is expected */ }
                    AccessibilityStatus::Blocked => {
                        eprintln!("in readonly paths, path {} expected to be readable, found non readable",path);
                        return;
                    }
                }
            }
        }
        match test_write_access(path) {
            std::io::Result::Err(e) => {
                eprintln!(
                    "in readonly paths, error in testing write access for path {} : {:?}",
                    path, e
                );
                return;
            }
            Ok(readability) => {
                match readability {
                    AccessibilityStatus::Accessible => {
                        eprintln!("in readonly paths, path {} expected to not be writable, found writable",path);
                        return;
                    }
                    AccessibilityStatus::Blocked => { /* This is expected */ }
                }
            }
        }
    }
}
