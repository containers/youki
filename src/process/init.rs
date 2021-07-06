use std::{io::Write, path::PathBuf};

use anyhow::Result;
use mio::unix::pipe::Sender;
use nix::{
    sched,
    unistd::{Gid, Uid},
};

use crate::{
    capabilities, command::Syscall, namespaces::Namespaces, process::message::Message, rootfs,
};

/// Contains sending end for pipe for the child process
pub struct InitProcess {
    sender_for_child: Sender,
}

impl InitProcess {
    /// create a new Init process structure
    pub fn new(sender_for_child: Sender) -> Self {
        Self { sender_for_child }
    }

    /// Notify that this process is ready
    // The child here is in perspective of overall hierarchy
    // main youki process -> child process -> init process
    // the child here does not mean child of the init process
    pub fn ready(&mut self) -> Result<()> {
        log::debug!(
            "init send to child {:?}",
            (Message::InitReady as u8).to_be_bytes()
        );
        self.write_message_for_child(Message::InitReady)?;
        Ok(())
    }

    #[inline]
    fn write_message_for_child(&mut self, msg: Message) -> Result<()> {
        self.sender_for_child
            .write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }
}

/// setup hostname, rootfs for the container process
pub fn setup_init_process(
    spec: &oci_spec::Spec,
    command: &impl Syscall,
    rootfs: PathBuf,
    namespaces: &Namespaces,
) -> Result<()> {
    let proc = &spec.process;

    command.set_hostname(spec.hostname.as_str())?;
    if proc.no_new_privileges {
        let _ = prctl::set_no_new_privileges(true);
    }

    rootfs::prepare_rootfs(
        &spec,
        &rootfs,
        namespaces
            .clone_flags
            .contains(sched::CloneFlags::CLONE_NEWUSER),
    )?;

    // change the root of filesystem of the process to the rootfs
    command.pivot_rootfs(&rootfs)?;

    command.set_id(Uid::from_raw(proc.user.uid), Gid::from_raw(proc.user.gid))?;
    capabilities::reset_effective(command)?;
    if let Some(caps) = &proc.capabilities {
        capabilities::drop_privileges(&caps, command)?;
    }
    Ok(())
}
