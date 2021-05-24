//! # Youki
//! Container Runtime written in Rust, inspired by [railcar](https://github.com/oracle/railcar)
//! This crate provides a container runtime which can be used by a high-level container runtime to run containers.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Clap;
use nix::sys::signal as nix_signal;

use youki::container::{Container, ContainerStatus};
use youki::create;
use youki::signal;
use youki::start;
use youki::utils;
use youki::{cgroups::v1::Manager, command::linux::LinuxCommand};

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
}

/// This is the entry point in the container runtime. The binary is run by a high-level container runtime,
/// with various flags passed. This parses the flags, creates and manages appropriate resources.
fn main() -> Result<()> {
    let opts = Opts::parse();

    if let Err(e) = youki::logger::init(opts.log) {
        eprintln!("log init failed: {:?}", e);
    }

    let root_path = PathBuf::from(&opts.root);
    fs::create_dir_all(&root_path)?;

    match opts.subcmd {
        SubCommand::Create(create) => create.exec(root_path, LinuxCommand),
        SubCommand::Start(start) => start.exec(root_path),
        SubCommand::Kill(kill) => {
            // resolves relative paths, symbolic links etc. and get complete path
            let root_path = fs::canonicalize(root_path)?;
            // state of container is stored in a directory named as container id inside
            // root directory given in commandline options
            let container_root = root_path.join(&kill.container_id);
            if !container_root.exists() {
                bail!("{} doesn't exists.", kill.container_id)
            }

            // load container state from json file, and check status of the container
            // it might be possible that kill is invoked on a already stopped container etc.
            let container = Container::load(container_root)?.refresh_status()?;
            if container.can_kill() {
                let sig = signal::from_str(kill.signal.as_str())?;
                log::debug!("kill signal {} to {}", sig, container.pid().unwrap());
                nix_signal::kill(container.pid().unwrap(), sig)?;
                container.update_status(ContainerStatus::Stopped)?.save()?;
                std::process::exit(0)
            } else {
                bail!(
                    "{} counld not be killed because it was {:?}",
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
                bail!("{} doesn't exists.", delete.container_id)
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
                    let cmanager = Manager::new(&cgroups_path)?;
                    cmanager.remove()?;
                }
                std::process::exit(0)
            } else {
                bail!(
                    "{} counld not be deleted because it was {:?}",
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
    }
}
