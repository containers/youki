//! # Youki
//! Container Runtime written in Rust, inspired by [railcar](https://github.com/oracle/railcar)
//! This crate provides a container runtime which can be used by a high-level container runtime to run containers.
mod commands;
mod logger;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::IntoApp;
use clap::{crate_version, Parser};
use nix::libc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::info;
use libcontainer::rootless::rootless_required;
use libcontainer::utils::create_dir_all_with_mode;
use nix::sys::stat::Mode;
use nix::unistd::getuid;

use liboci_cli::{CommonCmd, GlobalOpts, StandardCmd};

// High-level commandline option definition
// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Parser, Debug)]
#[clap(version = youki_version!(), author = env!("CARGO_PKG_AUTHORS"))]
struct Opts {
    #[clap(flatten)]
    global: GlobalOpts,

    #[clap(subcommand)]
    subcmd: SubCommand,
}

// Subcommands accepted by Youki, confirming with [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also for a short information, check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
#[derive(Parser, Debug)]
enum SubCommand {
    // Standard and common commands handled by the liboci_cli crate
    #[clap(flatten)]
    Standard(liboci_cli::StandardCmd),
    #[clap(flatten)]
    Common(liboci_cli::CommonCmd),

    // Youki specific extensions
    Info(info::Info),
    Completion(commands::completion::Completion),
}

/// output Youki version in Moby compatible format
#[macro_export]
macro_rules! youki_version {
    // For compatibility with Moby, match format here:
    // https://github.com/moby/moby/blob/65cc84abc522a564699bb171ca54ea1857256d10/daemon/info_unix.go#L280
    () => {
        concat!(
            "version ",
            crate_version!(),
            "\ncommit: ",
            crate_version!(),
            "-0-",
            env!("VERGEN_GIT_SHA_SHORT")
        )
    };
}

/// This is the entry point in the container runtime. The binary is run by a high-level container runtime,
/// with various flags passed. This parses the flags, creates and manages appropriate resources.
fn main() -> Result<()> {
    // A malicious container can gain access to the host machine by modifying youki's host
    // binary and infect it with malicious code. This vulnerability was first discovered
    // in runc and was assigned as CVE-2019-5736, but it also affects youki.
    //
    // The fix is to copy /proc/self/exe in an anonymous file descriptor (created via memfd_create),
    // seal it and re-execute it. Because the final step is re-execution, this needs to be done at
    // the beginning of this process.
    //
    // Ref: https://github.com/opencontainers/runc/commit/0a8e4117e7f715d5fbeef398405813ce8e88558b
    // Ref: https://github.com/lxc/lxc/commit/6400238d08cdf1ca20d49bafb85f4e224348bf9d
    pentacle::ensure_sealed().context("failed to seal /proc/self/exe")?;

    let opts = Opts::parse();
    let mut app = Opts::into_app();

    if let Err(e) = crate::logger::init(opts.global.debug, opts.global.log, opts.global.log_format)
    {
        eprintln!("log init failed: {:?}", e);
    }

    log::debug!(
        "started by user {} with {:?}",
        nix::unistd::geteuid(),
        std::env::args_os()
    );
    let root_path = determine_root_path(opts.global.root)?;
    let systemd_cgroup = opts.global.systemd_cgroup;

    match opts.subcmd {
        SubCommand::Standard(cmd) => match cmd {
            StandardCmd::Create(create) => {
                commands::create::create(create, root_path, systemd_cgroup)
            }
            StandardCmd::Start(start) => commands::start::start(start, root_path),
            StandardCmd::Kill(kill) => commands::kill::kill(kill, root_path),
            StandardCmd::Delete(delete) => commands::delete::delete(delete, root_path),
            StandardCmd::State(state) => commands::state::state(state, root_path),
        },
        SubCommand::Common(cmd) => match cmd {
            CommonCmd::Checkpointt(checkpoint) => {
                commands::checkpoint::checkpoint(checkpoint, root_path)
            }
            CommonCmd::Events(events) => commands::events::events(events, root_path),
            CommonCmd::Exec(exec) => commands::exec::exec(exec, root_path),
            CommonCmd::List(list) => commands::list::list(list, root_path),
            CommonCmd::Pause(pause) => commands::pause::pause(pause, root_path),
            CommonCmd::Ps(ps) => commands::ps::ps(ps, root_path),
            CommonCmd::Resume(resume) => commands::resume::resume(resume, root_path),
            CommonCmd::Run(run) => commands::run::run(run, root_path, systemd_cgroup),
            CommonCmd::Spec(spec) => commands::spec_json::spec(spec),
            CommonCmd::Update(update) => commands::update::update(update, root_path),
        },

        SubCommand::Info(info) => commands::info::info(info),
        SubCommand::Completion(completion) => {
            commands::completion::completion(completion, &mut app)
        }
    }
}

fn determine_root_path(root_path: Option<PathBuf>) -> Result<PathBuf> {
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

    let tmp_dir = PathBuf::from(format!("/tmp/youki-{}", uid));
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
    libcontainer::utils::get_temp_dir_path("default_youki_path")
}

#[cfg(not(test))]
fn get_default_rootless_path(uid: libc::uid_t) -> PathBuf {
    PathBuf::from(format!("/run/user/{}/youki", uid))
}

#[cfg(test)]
fn get_default_rootless_path(uid: libc::uid_t) -> PathBuf {
    libcontainer::utils::get_temp_dir_path(format!("default_rootless_youki_path_{}", uid).as_str())
}

#[cfg(test)]
mod tests {
    use crate::determine_root_path;
    use anyhow::{Context, Result};
    use libcontainer::utils::{get_temp_dir_path, TempDir};
    use nix::sys::stat::Mode;
    use nix::unistd::getuid;
    use std::fs;
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_determine_root_path_use_specified_by_user() -> Result<()> {
        // Create directory if it does not exist and return absolute path.
        let specified_path = get_temp_dir_path("provided_path");
        // Make sure directory does not exist.
        remove_dir(&specified_path)?;
        let non_abs_path = specified_path.join("../provided_path");
        let path = determine_root_path(Some(non_abs_path))
            .context("failed with specified path")?;
        assert_eq!(path, specified_path);

        // Return absolute path if directory exists.
        let specified_path = get_temp_dir_path("provided_path2");
        let _temp_dir = TempDir::new(&specified_path).context("failed to create temp dir")?;
        let non_abs_path = specified_path.join("../provided_path2");
        let path = determine_root_path(Some(non_abs_path))
            .context("failed with specified path")?;
        assert_eq!(path, specified_path);

        Ok(())
    }

    #[test]
    fn test_determine_root_path_non_rootless() -> Result<()> {
        // If we do not have root privileges skip the test as it will not succeed.
        if !getuid().is_root() {
            return Ok(());
        }

        let expected_path = get_temp_dir_path("default_youki_path");

        let path = determine_root_path(None).context("failed with default non rootless path")?;
        assert_eq!(path, expected_path);
        assert!(path.exists());

        fs::remove_dir(&expected_path).context("failed to remove dir")?;

        // Setup TempDir with invalid permissions so it is cleaned up after test.
        let _temp_dir = TempDir::new(&expected_path).context("failed to create temp dir")?;
        fs::set_permissions(&expected_path, Permissions::from_mode(Mode::S_IRUSR.bits()))
            .context("failed to set invalid permissions")?;

        assert!(determine_root_path(None).is_err());

        Ok(())
    }

    #[test]
    fn test_determine_root_path_rootless() -> Result<()> {
        std::env::set_var("YOUKI_USE_ROOTLESS", "true");

        // XDG_RUNTIME_DIR
        let xdg_dir = get_temp_dir_path("xdg_runtime");
        std::env::set_var("XDG_RUNTIME_DIR", &xdg_dir);
        let path = determine_root_path(None).context("failed with $XDG_RUNTIME_DIR path")?;
        assert_eq!(path, xdg_dir.join("youki"));
        assert!(path.exists());

        std::env::remove_var("XDG_RUNTIME_DIR");

        // Default rootless location
        let uid = getuid().as_raw();
        let default_rootless_path =
            get_temp_dir_path(format!("default_rootless_youki_path_{}", uid).as_str());
        // Create temp dir so it gets cleaned up. This is needed as we later switch permissions of this directory.
        let _temp_dir =
            TempDir::new(&default_rootless_path).context("failed to create temp dir")?;
        let path = determine_root_path(None).context("failed with default rootless path")?;
        assert_eq!(path, default_rootless_path);
        assert!(path.exists());

        // Set invalid permissions to default rootless path so that it fails for the next test.
        fs::set_permissions(
            default_rootless_path,
            Permissions::from_mode(Mode::S_IRUSR.bits()),
        )
        .context("failed to set invalid permissions")?;

        // Use HOME env var
        let home_path = get_temp_dir_path("youki_home");
        fs::create_dir_all(&home_path).context("failed to create fake home path")?;
        std::env::set_var("HOME", &home_path);
        let path = determine_root_path(None).context("failed with $HOME path")?;
        assert_eq!(path, home_path.join(".youki/run"));
        assert!(path.exists());

        std::env::remove_var("HOME");

        // Use temp dir
        let expected_temp_path = PathBuf::from(format!("/tmp/youki-{}", uid));
        // Create temp dir so it gets cleaned up. This is needed as we later switch permissions of this directory.
        let _temp_dir = TempDir::new(&expected_temp_path).context("failed to create temp dir")?;
        let path = determine_root_path(None).context("failed with temp path")?;
        assert_eq!(path, expected_temp_path);

        // Set invalid permissions to temp path so determine_root_path fails.
        fs::set_permissions(
            expected_temp_path,
            Permissions::from_mode(Mode::S_IRUSR.bits()),
        )
        .context("failed to set invalid permissions")?;

        assert!(determine_root_path(None).is_err());

        Ok(())
    }

    fn remove_dir(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_dir(path).context("failed to remove directory")?;
        }
        Ok(())
    }
}
