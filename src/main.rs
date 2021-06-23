//! # Youki
//! Container Runtime written in Rust, inspired by [railcar](https://github.com/oracle/railcar)
//! This crate provides a container runtime which can be used by a high-level container runtime to run containers.

use std::ffi::OsString;

use std::fs;
use std::io;
use std::io::Write;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use clap::Clap;
use nix::sys::signal as nix_signal;

use youki::command::linux::LinuxCommand;

use youki::container::{Container, ContainerStatus};
use youki::create;
use youki::info::{print_cgroups, print_hardware, print_kernel, print_os, print_youki};
use youki::rootless::should_use_rootless;
use youki::signal;
use youki::start;

use tabwriter::TabWriter;
use youki::cgroups;
use youki::utils;

/// High-level commandline option definition
/// This takes global options as well as individual commands as specified in [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
/// Also check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md) for more explanation
#[derive(Clap, Debug)]
#[clap(version = "1.0", author = "utam0k <k0ma@utam0k.jp>")]
struct Opts {
    /// root directory to store container state
    #[clap(short, long, default_value = "/run/youki")]
    root: PathBuf,
    #[clap(short, long)]
    log: Option<PathBuf>,
    #[clap(long)]
    log_format: Option<String>,
    /// Enable systemd cgroup manager, rather then use the cgroupfs directly.
    #[clap(short, long)]
    systemd_cgroup: bool,
    /// command to actually manage container
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap, Debug)]
pub struct Kill {
    container_id: String,
    signal: String,
}

#[derive(Clap, Debug)]
pub struct Delete {
    container_id: String,
    // forces deletion of the container.
    #[clap(short, long)]
    force: bool,
}

#[derive(Clap, Debug)]
pub struct StateArgs {
    pub container_id: String,
}

/// Subcommands accepted by Youki, confirming with [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
/// Also for a short information, check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
#[derive(Clap, Debug)]
enum SubCommand {
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Create(create::Create),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Start(start::Start),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Kill(Kill),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Delete(Delete),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    State(StateArgs),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Info,
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    List,
}

/// This is the entry point in the container runtime. The binary is run by a high-level container runtime,
/// with various flags passed. This parses the flags, creates and manages appropriate resources.
fn main() -> Result<()> {
    let opts = Opts::parse();

    if let Err(e) = youki::logger::init(opts.log) {
        eprintln!("log init failed: {:?}", e);
    }

    let root_path = if should_use_rootless() && opts.root.eq(&PathBuf::from("/run/youki")) {
        PathBuf::from("/tmp/rootless")
    } else {
        PathBuf::from(&opts.root)
    };
    fs::create_dir_all(&root_path)?;

    let systemd_cgroup = opts.systemd_cgroup;

    match opts.subcmd {
        SubCommand::Create(create) => create.exec(root_path, systemd_cgroup, LinuxCommand),
        SubCommand::Start(start) => start.exec(root_path),
        SubCommand::Kill(kill) => {
            // resolves relative paths, symbolic links etc. and get complete path
            let root_path = fs::canonicalize(root_path)?;
            // state of container is stored in a directory named as container id inside
            // root directory given in commandline options
            let container_root = root_path.join(&kill.container_id);
            if !container_root.exists() {
                bail!("{} doesn't exist.", kill.container_id)
            }

            // load container state from json file, and check status of the container
            // it might be possible that kill is invoked on a already stopped container etc.
            let container = Container::load(container_root)?.refresh_status()?;
            if container.can_kill() {
                let sig = signal::from_str(kill.signal.as_str())?;
                log::debug!("kill signal {} to {}", sig, container.pid().unwrap());
                nix_signal::kill(container.pid().unwrap(), sig)?;
                container.update_status(ContainerStatus::Stopped).save()?;
                std::process::exit(0)
            } else {
                bail!(
                    "{} could not be killed because it was {:?}",
                    container.id(),
                    container.status()
                )
            }
        }
        SubCommand::Delete(delete) => {
            log::debug!("start deleting {}", delete.container_id);
            // state of container is stored in a directory named as container id inside
            // root directory given in commandline options
            let container_root = root_path.join(&delete.container_id);
            if !container_root.exists() {
                bail!("{} doesn't exist.", delete.container_id)
            }
            // load container state from json file, and check status of the container
            // it might be possible that delete is invoked on a running container.
            log::debug!("load the container from {:?}", container_root);
            let container = Container::load(container_root)?.refresh_status()?;
            if container.can_delete() {
                if container.root.exists() {
                    nix::unistd::chdir(&PathBuf::from(&container.state.bundle))?;
                    let config_absolute_path = &PathBuf::from(&container.state.bundle)
                        .join(Path::new("config.json"))
                        .to_string_lossy()
                        .to_string();
                    log::debug!("load spec from {:?}", config_absolute_path);
                    let spec = oci_spec::Spec::load(config_absolute_path)?;
                    log::debug!("spec: {:?}", spec);

                    // remove the directory storing container state
                    log::debug!("remove dir {:?}", container.root);
                    fs::remove_dir_all(&container.root)?;

                    let cgroups_path =
                        utils::get_cgroup_path(&spec.linux.unwrap().cgroups_path, container.id());

                    // remove the cgroup created for the container
                    // check https://man7.org/linux/man-pages/man7/cgroups.7.html
                    // creating and removing cgroups section for more information on cgroups
                    let cmanager =
                        cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
                    cmanager.remove()?;
                }
                std::process::exit(0)
            } else {
                bail!(
                    "{} could not be deleted because it was {:?}",
                    container.id(),
                    container.status()
                )
            }
        }
        SubCommand::State(state_args) => {
            let root_path = fs::canonicalize(root_path)?;
            let container_root = root_path.join(state_args.container_id);
            let container = Container::load(container_root)?.refresh_status()?;
            println!("{}", serde_json::to_string_pretty(&container.state)?);
            std::process::exit(0);
        }

        SubCommand::Info => {
            print_youki();
            print_kernel();
            print_os();
            print_hardware();
            print_cgroups();

            Ok(())
        }

        SubCommand::List => {
            let root_path = fs::canonicalize(root_path)?;
            let mut content = String::new();

            for container_dir in fs::read_dir(root_path)? {
                let container_dir = container_dir?.path();
                let state_file = container_dir.join("state.json");
                if !state_file.exists() {
                    continue;
                }

                let container = Container::load(container_dir)?.refresh_status()?;
                let pid = if let Some(pid) = container.pid() {
                    pid.to_string()
                } else {
                    "".to_owned()
                };

                let user_name = if let Some(creator) = container.creator() {
                    creator
                } else {
                    OsString::new()
                };

                let created = if let Some(utc) = container.created() {
                    let local: DateTime<Local> = DateTime::from(utc);
                    local.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
                } else {
                    "".to_owned()
                };

                content.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\n",
                    container.id(),
                    pid,
                    container.status(),
                    container.bundle(),
                    created,
                    user_name.to_string_lossy()
                ));
            }

            let mut tab_writer = TabWriter::new(io::stdout());
            writeln!(&mut tab_writer, "ID\tPID\tSTATUS\tBUNDLE\tCREATED\tCREATOR")?;
            write!(&mut tab_writer, "{}", content)?;
            tab_writer.flush()?;

            Ok(())
        }
    }
}
