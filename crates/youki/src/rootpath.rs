use anyhow::{bail, Result};
use libcontainer::rootless::rootless_required;
use libcontainer::utils::create_dir_all_with_mode;
use nix::libc;
use nix::sys::stat::Mode;
use nix::unistd::getuid;
use std::fs;
use std::path::{Path, PathBuf};

pub fn determine(root_path: Option<PathBuf>) -> Result<PathBuf> {
    let uid = getuid().as_raw();

    if let Some(path) = root_path {
        if !path.exists() {
            create_dir_all_with_mode(&path, uid, Mode::S_IRWXU)?;
        }
        let path = path.canonicalize()?;
        return Ok(path);
    }

    if !rootless_required() {
        let path = get_default_not_rootless_path();
        create_dir_all_with_mode(&path, uid, Mode::S_IRWXU)?;
        return Ok(path);
    }

    // see https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
    if let Ok(path) = std::env::var("XDG_RUNTIME_DIR") {
        let path = Path::new(&path).join("youki");
        if create_dir_all_with_mode(&path, uid, Mode::S_IRWXU).is_ok() {
            return Ok(path);
        }
    }

    // XDG_RUNTIME_DIR is not set, try the usual location
    let path = get_default_rootless_path(uid);
    if create_dir_all_with_mode(&path, uid, Mode::S_IRWXU).is_ok() {
        return Ok(path);
    }

    if let Ok(path) = std::env::var("HOME") {
        if let Ok(resolved) = fs::canonicalize(path) {
            let run_dir = resolved.join(".youki/run");
            if create_dir_all_with_mode(&run_dir, uid, Mode::S_IRWXU).is_ok() {
                return Ok(run_dir);
            }
        }
    }

    let tmp_dir = PathBuf::from(format!("/tmp/youki-{uid}"));
    if create_dir_all_with_mode(&tmp_dir, uid, Mode::S_IRWXU).is_ok() {
        return Ok(tmp_dir);
    }

    bail!("could not find a storage location with suitable permissions for the current user");
}

#[cfg(not(test))]
fn get_default_not_rootless_path() -> PathBuf {
    PathBuf::from("/run/youki")
}

#[cfg(test)]
fn get_default_not_rootless_path() -> PathBuf {
    std::env::temp_dir().join("default_youki_path")
}

#[cfg(not(test))]
fn get_default_rootless_path(uid: libc::uid_t) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}/youki"))
}

#[cfg(test)]
fn get_default_rootless_path(uid: libc::uid_t) -> PathBuf {
    std::env::temp_dir().join(format!("default_rootless_youki_path_{uid}").as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    use nix::sys::stat::Mode;
    use nix::unistd::getuid;
    use std::fs;
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    fn test_user_specified() -> Result<()> {
        // If the user specifies a path that does not exist, we should
        // create it and return the absolute path.
        let tmp = tempfile::tempdir()?;
        // Note, the path doesn't exist yet because tempfile generated a random new empty dir.
        let specified_path = tmp.path().join("provided_path");
        let non_abs_path = specified_path.join("../provided_path");
        let path = determine(Some(non_abs_path)).context("failed with specified path")?;
        assert_eq!(path, specified_path);
        Ok(())
    }

    #[test]
    fn test_user_specified_exists() -> Result<()> {
        // If the user specifies a path that exists, we should return the
        // absolute path.
        let tmp = tempfile::tempdir()?;
        let specified_path = tmp.path().join("provided_path");
        std::fs::create_dir(&specified_path).context("failed to create dir")?;
        let non_abs_path = specified_path.join("../provided_path");
        let path = determine(Some(non_abs_path)).context("failed with specified path")?;
        assert_eq!(path, specified_path);

        Ok(())
    }

    #[test]
    fn test_determine_root_path_non_rootless() -> Result<()> {
        // If we do not have root privileges skip the test as it will not succeed.
        if !getuid().is_root() {
            return Ok(());
        }

        {
            let expected_path = super::get_default_not_rootless_path();
            let path = determine(None).context("failed with default non rootless path")?;
            assert_eq!(path, expected_path);
            assert!(path.exists());
            fs::remove_dir_all(&expected_path).context("failed to remove dir")?;
        }
        {
            let expected_path = get_default_not_rootless_path();
            fs::create_dir(&expected_path).context("failed to create dir")?;
            fs::set_permissions(&expected_path, Permissions::from_mode(Mode::S_IRUSR.bits()))
                .context("failed to set invalid permissions")?;
            assert!(determine(None).is_err());
            fs::remove_dir_all(&expected_path).context("failed to remove dir")?;
        }

        Ok(())
    }

    #[test]
    fn test_determine_root_path_rootless() -> Result<()> {
        std::env::set_var("YOUKI_USE_ROOTLESS", "true");

        // XDG_RUNTIME_DIR
        let tmp = tempfile::tempdir()?;
        let xdg_dir = tmp.path().join("xdg_runtime");
        std::env::set_var("XDG_RUNTIME_DIR", &xdg_dir);
        let path = determine(None).context("failed with $XDG_RUNTIME_DIR path")?;
        assert_eq!(path, xdg_dir.join("youki"));
        assert!(path.exists());
        std::env::remove_var("XDG_RUNTIME_DIR");

        // Default rootless location
        let uid = getuid().as_raw();
        let default_rootless_path = get_default_rootless_path(uid);
        scopeguard::defer!({
            let _ = fs::remove_dir_all(&default_rootless_path);
        });
        let path = determine(None).context("failed with default rootless path")?;
        assert_eq!(path, default_rootless_path);
        assert!(path.exists());

        // The `determine` function will default to the rootless default
        // path under `/run/user/$uid``. To test the `determine` function to
        // not use the default rootless path, we need to make the default
        // rootless path fail to create. So we set the path to an invalid
        // permission, so the `determine` function will use HOME env var or
        // `/tmp` directory instead.
        fs::set_permissions(
            &default_rootless_path,
            Permissions::from_mode(Mode::S_IRUSR.bits()),
        )
        .context("failed to set invalid permissions")?;

        // Use HOME env var
        let tmp = tempfile::tempdir()?;
        let home_path = tmp.path().join("youki_home");
        fs::create_dir_all(&home_path).context("failed to create fake home path")?;
        std::env::set_var("HOME", &home_path);
        let path = determine(None).context("failed with $HOME path")?;
        assert_eq!(path, home_path.join(".youki/run"));
        assert!(path.exists());
        std::env::remove_var("HOME");

        // Use /tmp dir
        let uid = getuid().as_raw();
        let expected_temp_path = PathBuf::from(format!("/tmp/youki-{uid}"));
        let path = determine(None).context("failed with temp path")?;
        assert_eq!(path, expected_temp_path);
        // Set invalid permissions to temp path so determine_root_path fails.
        fs::set_permissions(
            &expected_temp_path,
            Permissions::from_mode(Mode::S_IRUSR.bits()),
        )
        .context("failed to set invalid permissions")?;
        assert!(determine(None).is_err());
        fs::remove_dir_all(&expected_temp_path).context("failed to remove dir")?;

        Ok(())
    }
}
