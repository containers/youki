use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Clap;
use nix::sys::signal as nix_signal;

use youki::container::{Container, ContainerStatus};
use youki::create;
use youki::signal;
use youki::start;

#[derive(Clap, Debug)]
#[clap(version = "1.0", author = "utam0k <k0ma@utam0k.jp>")]
struct Opts {
    #[clap(short, long, default_value = "/run/youki")]
    root: PathBuf,
    #[clap(short, long)]
    log: Option<PathBuf>,
    #[clap(long)]
    log_format: Option<String>,
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

impl SubCommand {
    fn get_container_id(&self) -> &String {
        match &self {
            SubCommand::Create(create) => &create.container_id,
            SubCommand::Start(start) => &start.container_id,
            SubCommand::Delete(delete) => &delete.container_id,
            SubCommand::Kill(kill) => &kill.container_id,
            SubCommand::State(state_args) => &state_args.container_id,
        }
    }
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    youki::logger::init(opts.subcmd.get_container_id().as_str(), opts.log)?;
    log::debug!("Hello, world");

    let root_path = PathBuf::from(&opts.root);
    fs::create_dir_all(&root_path)?;

    match opts.subcmd {
        SubCommand::Create(create) => create.exec(root_path),
        SubCommand::Start(start) => start.exec(root_path),
        SubCommand::Kill(kill) => {
            let root_path = fs::canonicalize(root_path)?;
            let container_root = root_path.join(&kill.container_id);
            if !container_root.exists() {
                bail!("{} doesn't exists.", kill.container_id)
            }
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
            let container_root = root_path.join(&delete.container_id);
            if !container_root.exists() {
                bail!("{} doesn't exists.", delete.container_id)
            }
            let container = Container::load(container_root)?.refresh_status()?;
            if container.can_delete() {
                if container.root.exists() {
                    fs::remove_dir_all(&container.root)?;
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
