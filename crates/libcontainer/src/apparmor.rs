use crate::utils;
use std::{
    fs::{self},
    path::Path,
};

#[derive(Debug, thiserror::Error)]
pub enum AppArmorError {
    #[error("failed to apply AppArmor profile")]
    ApplyProfile {
        path: std::path::PathBuf,
        profile: String,
        // TODO: fix this after `utils` crate is migrated to `thiserror`
        source: anyhow::Error,
    },
    #[error("failed to read AppArmor profile: {source} {path}")]
    ReadProfile {
        path: String,
        source: std::io::Error,
    },
}

type Result<T> = std::result::Result<T, AppArmorError>;

const ENABLED_PARAMETER_PATH: &str = "/sys/module/apparmor/parameters/enabled";

/// Checks if AppArmor has been enabled on the system.
pub fn is_enabled() -> Result<bool> {
    let aa_enabled =
        fs::read_to_string(ENABLED_PARAMETER_PATH).map_err(|e| AppArmorError::ReadProfile {
            path: ENABLED_PARAMETER_PATH.to_string(),
            source: e,
        })?;
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
    utils::ensure_procfs(path).map_err(|err| AppArmorError::ApplyProfile {
        path: path.to_owned(),
        profile: profile.to_owned(),
        source: err,
    })?;
    utils::write_file(path, format!("exec {profile}")).map_err(|err| AppArmorError::ApplyProfile {
        path: path.to_owned(),
        profile: profile.to_owned(),
        source: err,
    })
}
