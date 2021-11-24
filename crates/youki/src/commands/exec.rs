use anyhow::Result;
use clap::Parser;
use std::{error::Error, path::PathBuf};

use libcontainer::{container::builder::ContainerBuilder, syscall::syscall::create_syscall};

/// Execute a process within an existing container
#[derive(Parser, Debug)]
pub struct Exec {
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(long)]
    pub console_socket: Option<PathBuf>,
    #[clap(short, long)]
    pub tty: bool,
    #[clap(long)]
    /// Current working directory of the container
    pub cwd: Option<PathBuf>,
    #[clap(long)]
    /// The file to which the pid of the container process should be written to
    pub pid_file: Option<PathBuf>,
    /// Environment variables that should be set in the container
    #[clap(short, long, parse(try_from_str = parse_key_val), number_of_values = 1)]
    pub env: Vec<(String, String)>,
    /// Prevent the process from gaining additional privileges
    #[clap(long)]
    pub no_new_privs: bool,
    /// Path to process.json
    #[clap(short, long)]
    pub process: Option<PathBuf>,
    /// Detach from the container process
    #[clap(short, long)]
    pub detach: bool,
    /// Identifier of the container
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
    /// Command that should be executed in the container
    #[clap(required = false)]
    pub command: Vec<String>,
}

pub fn exec(args: Exec, root_path: PathBuf) -> Result<()> {
    let syscall = create_syscall();
    ContainerBuilder::new(args.container_id.clone(), syscall.as_ref())
        .with_root_path(root_path)
        .with_console_socket(args.console_socket.as_ref())
        .with_pid_file(args.pid_file.as_ref())
        .as_tenant()
        .with_cwd(args.cwd.as_ref())
        .with_env(args.env.clone().into_iter().collect())
        .with_process(args.process.as_ref())
        .with_no_new_privs(args.no_new_privs)
        .with_process(args.process.as_ref())
        .with_container_args(args.command.clone())
        .build()
}

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
