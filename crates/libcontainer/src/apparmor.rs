use anyhow::{Context, Result};
use std::{fs, path::Path};

use crate::utils;

const ENABLED_PARAMETER_PATH: &str = "/sys/module/apparmor/parameters/enabled";

/// Checks if AppArmor has been enabled on the system.
pub fn is_enabled() -> Result<bool> {
    let aa_enabled = fs::read_to_string(ENABLED_PARAMETER_PATH)
        .with_context(|| format!("could not read {}", ENABLED_PARAMETER_PATH))?;
    Ok(aa_enabled.starts_with('Y'))
}

/// Applies an AppArmor profile to the container.
pub fn apply_profile(profile: &str) -> Result<()> {
    if profile.is_empty() {
        return Ok(());
    }

    // Try the module specific subdirectory. This is the recommended way to configure
    // LSMs since Linux 5.1. AppArmor has such a directory since Linux 5.8.
    if activate_profile(Path::new("/proc/self/attr/apparmor/exec"), profile).is_ok() {
        return Ok(());
    }

    // try the legacy interface
    activate_profile(Path::new("/proc/self/attr/exec"), profile)
}

fn activate_profile(path: &Path, profile: &str) -> Result<()> {
    utils::ensure_procfs(path)?;
    utils::write_file(path, format!("exec {}", profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    #[serial]
    #[test]
    fn test_apparmor_is_enabled() -> Result<()> {
        if let Err(e) = fs::File::open("/sys/kernel/security/apparmor") {
            if e.kind() == std::io::ErrorKind::NotFound && is_enabled()? {
                // from runc it checks /sys/kernel/security/apparmor exists or not,
                // if that path isn't exist then ENABLED_PARAMETER_PATH should be false too.
                assert!(false)
            }
        }
        Ok(())
    }
}
