use anyhow::Result;
use nix::sys::wait::{waitpid, WaitStatus};
use std::path::PathBuf;

use libcontainer::{container::builder::ContainerBuilder, syscall::syscall::create_syscall};
use liboci_cli::Exec;

pub fn exec(args: Exec, root_path: PathBuf) -> Result<i32> {
    let syscall = create_syscall();
    let pid = ContainerBuilder::new(args.container_id.clone(), syscall.as_ref())
        .with_root_path(root_path)?
        .with_console_socket(args.console_socket.as_ref())
        .with_pid_file(args.pid_file.as_ref())?
        .as_tenant()
        .with_cwd(args.cwd.as_ref())
        .with_env(args.env.clone().into_iter().collect())
        .with_process(args.process.as_ref())
        .with_no_new_privs(args.no_new_privs)
        .with_container_args(args.command.clone())
        .build()?;

    // do not wait if detach is given, 
    // although will still report exec errors from above '?'
    // TODO add better explanation here
    if args.detach{
        return Ok(0);
    }

    match waitpid(pid, None)? {
        WaitStatus::Exited(_, status) => Ok(status),
        WaitStatus::Signaled(_, sig, _) => Ok(sig as i32),
        _ => Ok(0),
    }
}
