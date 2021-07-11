//! # Youki
//! Container Runtime written in Rust, inspired by [railcar](https://github.com/oracle/railcar)
//! This crate provides a container runtime which can be used by a high-level container runtime to run containers.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Clap;

use youki::create;
use youki::delete;
use youki::info;
use youki::kill;
use youki::list;
use youki::rootless::should_use_rootless;
use youki::start;
use youki::state;

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

/// Subcommands accepted by Youki, confirming with [OCI runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
/// Also for a short information, check [runc commandline documentation](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
#[derive(Clap, Debug)]
enum SubCommand {
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Create(create::Create),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Start(start::Start),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Kill(kill::Kill),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Delete(delete::Delete),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    State(state::State),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    Info(info::Info),
    #[clap(version = "0.0.1", author = "utam0k <k0ma@utam0k.jp>")]
    List(list::List),
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
        SubCommand::Create(create) => create.exec(root_path, systemd_cgroup),
        SubCommand::Start(start) => start.exec(root_path),
        SubCommand::Kill(kill) => kill.exec(root_path),
        SubCommand::Delete(delete) => delete.exec(root_path, systemd_cgroup),
        SubCommand::State(state) => state.exec(root_path),
        SubCommand::Info(info) => info.exec(),
        SubCommand::List(list) => list.exec(root_path),
    }
}
