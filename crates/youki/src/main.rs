//! # Youki
//! Container Runtime written in Rust, inspired by [railcar](https://github.com/oracle/railcar)
//! This crate provides a container runtime which can be used by a high-level container runtime to run containers.
mod commands;
mod logger;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::{crate_version, Parser};

use crate::commands::info;
use libcontainer::rootless::rootless_required;
use libcontainer::utils;
use libcontainer::utils::create_dir_all_with_mode;
use nix::sys::stat::Mode;
use nix::unistd::getuid;

// High-level commandline option definition
// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Parser, Debug)]
#[clap(version = crate_version!(), author = "youki team")]
struct Opts {
    /// change log level to debug.
    // Example in future : '--debug     change log level to debug. (default: "warn")'
    #[clap(long)]
    debug: bool,
    #[clap(short, long)]
    log: Option<PathBuf>,
    #[clap(long)]
    log_format: Option<String>,
    /// root directory to store container state
    #[clap(short, long)]
    root: Option<PathBuf>,
    /// Enable systemd cgroup manager, rather then use the cgroupfs directly.
    #[clap(short, long)]
    systemd_cgroup: bool,
    /// command to actually manage container
    #[clap(subcommand)]
    subcmd: SubCommand,
}

// Subcommands accepted by Youki, confirming with [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// Also for a short information, check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
#[derive(Parser, Debug)]
enum SubCommand {
    Create(liboci_cli::Create),
    Start(liboci_cli::Start),
    State(liboci_cli::State),
    Kill(liboci_cli::Kill),
    Delete(liboci_cli::Delete),

    Events(liboci_cli::Events),
    Exec(liboci_cli::Exec),
    List(liboci_cli::List),
    Pause(liboci_cli::Pause),
    #[clap(setting=clap::AppSettings::AllowLeadingHyphen)]
    Ps(liboci_cli::Ps),
    Resume(liboci_cli::Resume),
    Run(liboci_cli::Run),
    Spec(liboci_cli::Spec),

    Info(info::Info),
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

    if let Err(e) = crate::logger::init(opts.debug, opts.log, opts.log_format) {
        eprintln!("log init failed: {:?}", e);
    }

    log::debug!(
        "started by user {} with {:?}",
        nix::unistd::geteuid(),
        std::env::args_os()
    );
    let root_path = determine_root_path(opts.root)?;
    let systemd_cgroup = opts.systemd_cgroup;

    match opts.subcmd {
        SubCommand::Create(create) => commands::create::create(create, root_path, systemd_cgroup),
        SubCommand::Start(start) => commands::start::start(start, root_path),
        SubCommand::Run(run) => commands::run::run(run, root_path, systemd_cgroup),
        SubCommand::Exec(exec) => commands::exec::exec(exec, root_path),
        SubCommand::Kill(kill) => commands::kill::kill(kill, root_path),
        SubCommand::Delete(delete) => commands::delete::delete(delete, root_path),
        SubCommand::State(state) => commands::state::state(state, root_path),
        SubCommand::Info(info) => commands::info::info(info),
        SubCommand::List(list) => commands::list::list(list, root_path),
        SubCommand::Spec(spec) => commands::spec_json::spec(spec),
        SubCommand::Pause(pause) => commands::pause::pause(pause, root_path),
        SubCommand::Resume(resume) => commands::resume::resume(resume, root_path),
        SubCommand::Events(events) => commands::events::events(events, root_path),
        SubCommand::Ps(ps) => commands::ps::ps(ps, root_path),
    }
}

fn determine_root_path(root_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = root_path {
        return Ok(path);
    }

    if !rootless_required() {
        let default = PathBuf::from("/run/youki");
        utils::create_dir_all(&default)?;
        return Ok(default);
    }

    // see https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
    let uid = getuid().as_raw();
    if let Ok(path) = std::env::var("XDG_RUNTIME_DIR") {
        let path = Path::new(&path).join("youki");
        if create_dir_all_with_mode(&path, uid, Mode::S_IRWXU).is_ok() {
            return Ok(path);
        }
    }

    // XDG_RUNTIME_DIR is not set, try the usual location
    let path = PathBuf::from(format!("/run/user/{}/youki", uid));
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
