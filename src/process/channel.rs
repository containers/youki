use crate::process::message::Message;
use crate::rootless::Rootless;
use crate::utils;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use mio::unix::pipe;
use mio::unix::pipe::{Receiver, Sender};
use mio::{Events, Interest, Poll, Token};
use nix::unistd::Pid;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Maximum event capacity of polling
const MAX_EVENTS: usize = 128;
/// Time to wait when polling for message from child process
const WAIT_FOR_CHILD: Duration = Duration::from_secs(5);
/// Time to wait when polling for mapping ack from parent
const WAIT_FOR_MAPPING: Duration = Duration::from_secs(3);
// Token is used to identify which socket generated an event
const PARENT: Token = Token(0);

pub struct Channel {
    sender: Sender,
    receiver: Receiver,
    poll: Poll,
}

impl Channel {
    pub fn new() -> Result<Self> {
        let poll = Poll::new()?;
        let (sender, mut receiver) = pipe::new()?;
        poll.registry()
            .register(&mut receiver, PARENT, Interest::READABLE)?;

        Ok(Self {
            sender,
            receiver,
            poll,
        })
    }

    pub fn send_child_ready(&mut self, pid: Pid) -> Result<()> {
        // Send over the ChildReady follow by the pid.
        log::debug!("sending init pid ({:?})", pid);
        self.write_message(Message::ChildReady)?;
        self.sender.write_all(&(pid.as_raw()).to_be_bytes())?;
        Ok(())
    }

    // requests the parent to write the id mappings for the child process
    // this needs to be done from the parent see https://man7.org/linux/man-pages/man7/user_namespaces.7.html
    pub fn send_identifier_mapping_request(&mut self) -> Result<()> {
        log::debug!("send identifier mapping request");
        self.write_message(Message::WriteMapping)?;
        Ok(())
    }

    pub fn send_mapping_written(&mut self) -> Result<()> {
        log::debug!("identifier mapping written");
        self.sender
            .write_all(&(Message::MappingWritten as u8).to_be_bytes())?;
        Ok(())
    }

    // wait until the parent process has finished writing the id mappings
    pub fn wait_for_mapping_ack(&mut self) -> Result<()> {
        let mut events = Events::with_capacity(MAX_EVENTS);
        log::debug!("waiting for mapping ack");

        self.poll.poll(&mut events, Some(WAIT_FOR_MAPPING))?;
        for event in events.iter() {
            if event.token() == PARENT {
                let mut buf = [0; 1];
                if let Err(e) = self.receiver.read_exact(&mut buf) {
                    if e.kind() != ErrorKind::WouldBlock {
                        bail!(
                            "Failed to receive a message from the child process. {:?}",
                            e
                        )
                    }
                }

                match Message::from(u8::from_be_bytes(buf)) {
                    Message::MappingWritten => return Ok(()),
                    msg => bail!(
                        "receive unexpected message {:?} in waiting for mapping ack",
                        msg
                    ),
                }
            } else {
                unreachable!();
            }
        }

        unreachable!("timed out waiting for mapping ack")
    }

    pub fn wait_for_mapping_request(
        &mut self,
        child_pid: Pid,
        rootless: Option<&Rootless>,
        callback: &mut Channel,
    ) -> Result<()> {
        // Create collection with capacity to store up to MAX_EVENTS events
        let mut events = Events::with_capacity(MAX_EVENTS);
        loop {
            // poll the receiving end of pipe created for WAIT_FOR_CHILD duration for an event
            self.poll.poll(&mut events, Some(WAIT_FOR_CHILD))?;
            for event in events.iter() {
                if event.token() == PARENT {
                    // read data from pipe
                    let mut buf = [0; 1];
                    if let Err(e) = self.receiver.read_exact(&mut buf) {
                        if e.kind() != ErrorKind::WouldBlock {
                            bail!(
                                "Failed to receive a message from the child process. {:?}",
                                e
                            )
                        }
                    }

                    // convert to Message wrapper
                    match Message::from(u8::from_be_bytes(buf)) {
                        Message::WriteMapping => {
                            log::debug!("write mapping for pid {:?}", child_pid);
                            utils::write_file(format!("/proc/{}/setgroups", child_pid), "deny")?;
                            write_uid_mapping(child_pid, rootless)?;
                            write_gid_mapping(child_pid, rootless)?;
                            callback.send_mapping_written()?;
                            return Ok(());
                        }
                        msg => bail!(
                            "receive unexpected message {:?} waiting for mapping request",
                            msg
                        ),
                    }
                } else {
                    unreachable!();
                }
            }
        }
    }

    /// Waits for associated child process to send ready message
    /// and return the pid of init process which is forked by child process
    pub fn wait_for_child_ready(&mut self) -> Result<Pid> {
        // Create collection with capacity to store up to MAX_EVENTS events
        let mut events = Events::with_capacity(MAX_EVENTS);
        loop {
            // poll the receiving end of pipe created for WAIT_FOR_CHILD duration for an event
            self.poll.poll(&mut events, Some(WAIT_FOR_CHILD))?;
            for event in events.iter() {
                if event.token() == PARENT {
                    // read data from pipe
                    let mut buf = [0; 1];
                    if let Err(e) = self.receiver.read_exact(&mut buf) {
                        if e.kind() != ErrorKind::WouldBlock {
                            bail!(
                                "Failed to receive a message from the child process. {:?}",
                                e
                            )
                        }
                    }

                    // convert to Message wrapper
                    match Message::from(u8::from_be_bytes(buf)) {
                        Message::ChildReady => {
                            log::debug!("received child ready message");
                            let mut buf = [0; 4];
                            if let Err(e) = self.receiver.read_exact(&mut buf) {
                                if e.kind() != ErrorKind::WouldBlock {
                                    bail!(
                                        "Failed to receive a message from the child process. {:?}",
                                        e
                                    )
                                }
                            }

                            return Ok(Pid::from_raw(i32::from_be_bytes(buf)));
                        }
                        msg => bail!(
                            "receive unexpected message {:?} waiting for child ready",
                            msg
                        ),
                    }
                } else {
                    unreachable!();
                }
            }
        }
    }

    #[inline]
    fn write_message(&mut self, msg: Message) -> Result<()> {
        self.sender.write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }
}

fn write_uid_mapping(target_pid: Pid, rootless: Option<&Rootless>) -> Result<()> {
    if let Some(rootless) = rootless {
        if let Some(uid_mappings) = rootless.gid_mappings {
            return write_id_mapping(
                &format!("/proc/{}/uid_map", target_pid),
                uid_mappings,
                rootless.newuidmap.as_deref(),
            );
        }
    }

    Ok(())
}

fn write_gid_mapping(target_pid: Pid, rootless: Option<&Rootless>) -> Result<()> {
    if let Some(rootless) = rootless {
        if let Some(gid_mappings) = rootless.gid_mappings {
            return write_id_mapping(
                &format!("/proc/{}/gid_map", target_pid),
                gid_mappings,
                rootless.newgidmap.as_deref(),
            );
        }
    }

    Ok(())
}

fn write_id_mapping(
    map_file: &str,
    mappings: &[oci_spec::LinuxIdMapping],
    map_binary: Option<&Path>,
) -> Result<()> {
    let mappings: Vec<String> = mappings
        .iter()
        .map(|m| format!("{} {} {}", m.container_id, m.host_id, m.size))
        .collect();
    if mappings.len() == 1 {
        utils::write_file(map_file, mappings.first().unwrap())?;
    } else {
        Command::new(map_binary.unwrap())
            .args(mappings)
            .output()
            .with_context(|| format!("failed to execute {:?}", map_binary))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::wait;
    use nix::unistd;

    #[test]
    fn test_channel_child_ready() -> Result<()> {
        let ch = &mut Channel::new()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                wait::waitpid(child, None)?;
                let pid = ch.wait_for_child_ready()?;
                assert_eq!(pid, child);
            }
            unistd::ForkResult::Child => {
                let pid = unistd::getpid();
                ch.send_child_ready(pid)?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    fn test_channel_id_mapping() -> Result<()> {
        let ch = &mut Channel::new()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                ch.wait_for_mapping_ack()?;
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                ch.send_mapping_written()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}
