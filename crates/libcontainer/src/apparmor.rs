use std::fs::{self};
use std::path::Path;

use crate::utils;

#[derive(Debug, thiserror::Error)]
pub enum AppArmorError {
    #[error("failed to apply AppArmor profile")]
    ActivateProfile {
        path: std::path::PathBuf,
        profile: String,
        source: std::io::Error,
    },
    #[error(transparent)]
    EnsureProcfs(#[from] utils::EnsureProcfsError),
}

type Result<T> = std::result::Result<T, AppArmorError>;

const ENABLED_PARAMETER_PATH: &str = "/sys/module/apparmor/parameters/enabled";

/// Checks if AppArmor has been enabled on the system.
pub fn is_enabled() -> std::result::Result<bool, std::io::Error> {
    let aa_enabled = fs::read_to_string(ENABLED_PARAMETER_PATH)?;
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
    utils::ensure_procfs(path).map_err(AppArmorError::EnsureProcfs)?;
    fs::write(path, format!("exec {profile}")).map_err(|err| AppArmorError::ActivateProfile {
        path: path.to_owned(),
        profile: profile.to_owned(),
        source: err,
    })
}
