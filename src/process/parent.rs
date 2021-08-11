use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::{MAX_EVENTS, WAIT_FOR_CHILD};
use crate::process::message::Message;
use crate::process::WAIT_FOR_MAPPING;
use crate::rootless::Rootless;
use crate::utils;
use anyhow::Context;
use anyhow::{bail, Result};
use mio::unix::pipe;
use mio::unix::pipe::{Receiver, Sender};
use mio::{Events, Interest, Poll, Token};
use nix::unistd::Pid;
use oci_spec::LinuxIdMapping;

// Token is used to identify which socket generated an event
const PARENT: Token = Token(0);

/// Contains receiving end of pipe to child process and a poller for that.
pub struct ParentProcess<'a> {
    child_channel: ChildChannel<'a>,
}

// Poll is used to register and listen for various events
// by registering it with an event source such as receiving end of a pipe
impl<'a> ParentProcess<'a> {
    /// Create new Parent process structure
    pub fn new(rootless: &'a Option<Rootless>) -> Result<(Self, ParentChannel)> {
        let (parent_channel, child_channel) = Self::setup_pipes(rootless)?;
        let parent = Self { child_channel };

        Ok((parent, parent_channel))
    }

    fn setup_pipes(rootless: &'a Option<Rootless>) -> Result<(ParentChannel, ChildChannel<'a>)> {
        let (send_to_parent, receive_from_child) = pipe::new()?;
        let (send_to_child, receive_from_parent) = pipe::new()?;

        let parent_channel = ParentChannel::new(send_to_parent, receive_from_parent)?;
        let child_channel = ChildChannel::new(send_to_child, receive_from_child, rootless)?;

        Ok((parent_channel, child_channel))
    }

    /// Waits for associated child process to send ready message
    /// and return the pid of init process which is forked by child process
    pub fn wait_for_child_ready(&mut self, child_pid: Pid) -> Result<()> {
        self.child_channel.wait_for_child_ready(child_pid)?;
        Ok(())
    }
}

// Channel for communicating with the parent
pub struct ParentChannel {
    sender: Sender,
    receiver: Receiver,
    poll: Poll,
}

impl ParentChannel {
    fn new(sender: Sender, mut receiver: Receiver) -> Result<Self> {
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, PARENT, Interest::READABLE)?;
        Ok(Self {
            sender,
            receiver,
            poll,
        })
    }

    pub fn send_child_ready(&mut self) -> Result<()> {
        // write ChildReady message to the pipe to parent
        log::debug!("[child to parent] sending child ready");
        self.write_message(Message::ChildReady)?;
        Ok(())
    }

    // requests the parent to write the id mappings for the child process
    // this needs to be done from the parent see https://man7.org/linux/man-pages/man7/user_namespaces.7.html
    pub fn request_identifier_mapping(&mut self) -> Result<()> {
        log::debug!("[child to parent] request identifier mapping");
        self.write_message(Message::WriteMapping)?;
        Ok(())
    }

    // wait until the parent process has finished writing the id mappings
    pub fn wait_for_mapping_ack(&mut self) -> Result<()> {
        let mut events = Events::with_capacity(MAX_EVENTS);
        log::debug!("waiting for ack from parent");

        self.poll.poll(&mut events, Some(WAIT_FOR_MAPPING))?;
        for event in events.iter() {
            if event.token() == PARENT {
                let mut buf = [0; 1];
                match self.receiver.read_exact(&mut buf) {
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => (),
                    Err(e) => bail!(
                        "Failed to receive a message from the child process. {:?}",
                        e
                    ),
                    _ => (),
                }

                match Message::from(u8::from_be_bytes(buf)) {
                    Message::MappingWritten => return Ok(()),
                    msg => bail!("receive unexpected message {:?} in child process", msg),
                }
            }
        }
        unreachable!("timed out waiting for mapping ack from parent")
    }

    #[inline]
    fn write_message(&mut self, msg: Message) -> Result<()> {
        self.sender.write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }
}

struct ChildChannel<'a> {
    sender: Sender,
    receiver: Receiver,
    poll: Poll,
    rootless: &'a Option<Rootless<'a>>,
}

impl<'a> ChildChannel<'a> {
    fn new(sender: Sender, mut receiver: Receiver, rootless: &'a Option<Rootless>) -> Result<Self> {
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, PARENT, Interest::READABLE)?;
        Ok(Self {
            sender,
            receiver,
            poll,
            rootless,
        })
    }

    /// Waits for associated child process to send ready message
    /// and return the pid of init process which is forked by child process
    pub fn wait_for_child_ready(&mut self, child_pid: Pid) -> Result<()> {
        // Create collection with capacity to store up to MAX_EVENTS events
        let mut events = Events::with_capacity(MAX_EVENTS);
        loop {
            // poll the receiving end of pipe created for WAIT_FOR_CHILD duration for an event
            self.poll.poll(&mut events, Some(WAIT_FOR_CHILD))?;
            for event in events.iter() {
                // check if the event token in PARENT
                // note that this does not assign anything to PARENT, but instead compares PARENT and event.token()
                // check http://patshaughnessy.net/2018/1/18/learning-rust-if-let-vs--match for a bit more detailed explanation
                if let PARENT = event.token() {
                    // read data from pipe
                    let mut buf = [0; 1];
                    match self.receiver.read_exact(&mut buf) {
                        // This error simply means that there are no more incoming connections waiting to be accepted at this point.
                        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                            break;
                        }
                        Err(e) => bail!(
                            "Failed to receive a message from the child process. {:?}",
                            e
                        ),
                        _ => (),
                    };
                    // convert to Message wrapper
                    match Message::from(u8::from_be_bytes(buf)) {
                        Message::ChildReady => {
                            log::debug!("received child ready message");
                            return Ok(());
                        }
                        Message::WriteMapping => {
                            log::debug!("write mapping for pid {:?}", child_pid);
                            utils::write_file(format!("/proc/{}/setgroups", child_pid), "deny")?;
                            self.write_uid_mapping(child_pid)?;
                            self.write_gid_mapping(child_pid)?;
                            self.notify_mapping_written()?;
                        }
                        msg => bail!("receive unexpected message {:?} in parent process", msg),
                    }
                } else {
                    // as the poll is registered with only parent token
                    unreachable!()
                }
            }
        }
    }

    fn notify_mapping_written(&mut self) -> Result<()> {
        self.sender
            .write_all(&(Message::MappingWritten as u8).to_be_bytes())?;
        Ok(())
    }

    fn write_uid_mapping(&self, target_pid: Pid) -> Result<()> {
        if let Some(rootless) = self.rootless.as_ref() {
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

    fn write_gid_mapping(&self, target_pid: Pid) -> Result<()> {
        if let Some(rootless) = self.rootless.as_ref() {
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
}

fn write_id_mapping(
    map_file: &str,
    mappings: &[LinuxIdMapping],
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
