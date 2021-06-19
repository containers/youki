use std::io::ErrorKind;
use std::io::Read;

use anyhow::{bail, Result};
use mio::unix::pipe;
use mio::unix::pipe::Receiver;
use mio::unix::pipe::Sender;
use mio::{Events, Interest, Poll, Token};
use nix::unistd::Pid;

use super::parent::ParentChannel;
use super::{MAX_EVENTS, WAIT_FOR_INIT};
use crate::process::message::Message;

// Token is used to identify which socket generated an event
const CHILD: Token = Token(1);

/// Contains sending end of pipe for parent process, receiving end of pipe
/// for the init process and poller for that
pub struct ChildProcess {
    parent_channel: ParentChannel,
    receiver: Option<Receiver>,
    poll: Option<Poll>,
}

// Note : The original youki process first forks into 'parent' (P) and 'child' (C1) process
// of which this represents the child (C1) process. The C1 then again forks into parent process which is C1,
// and Child (C2) process. C2 is called as init process as it will run the command of the container. But form
// a process point of view, init process is child of child process, which is child of original youki process.
impl ChildProcess {
    /// create a new Child process structure
    pub fn new(parent_channel: ParentChannel) -> Result<Self> {
        Ok(Self {
            parent_channel,
            receiver: None,
            poll: None,
        })
    }

    /// sets up sockets for init process
    pub fn setup_pipe(&mut self) -> Result<Sender> {
        // create a new pipe
        let (sender, mut receiver) = pipe::new()?;
        // create a new poll, and register the receiving end of pipe to it
        // This will poll for the read events, so when data is written to sending end of the pipe,
        // the receiving end will be readable and poll wil notify
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, CHILD, Interest::READABLE)?;

        self.receiver = Some(receiver);
        self.poll = Some(poll);
        Ok(sender)
    }

    /// Indicate that child process has forked the init process to parent process
    pub fn notify_parent(&mut self, init_pid: Pid) -> Result<()> {
        self.parent_channel.send_init_pid(init_pid)?;
        Ok(())
    }

    pub fn request_identifier_mapping(&mut self) -> Result<()> {
        self.parent_channel.request_identifier_mapping()?;
        Ok(())
    }

    pub fn wait_for_mapping_ack(&mut self) -> Result<()> {
        self.parent_channel.wait_for_mapping_ack()?;
        Ok(())
    }

    /// Wait for the init process to be ready
    pub fn wait_for_init_ready(&mut self) -> Result<()> {
        // make sure pipe for init process is set up
        let receiver = self
            .receiver
            .as_mut()
            .expect("Complete the setup of uds in advance.");
        let poll = self
            .poll
            .as_mut()
            .expect("Complete the setup of uds in advance.");

        // Create collection with capacity to store up to MAX_EVENTS events
        let mut events = Events::with_capacity(MAX_EVENTS);
        // poll the receiving end of pipe created for WAIT_FOR_INIT duration an event
        poll.poll(&mut events, Some(WAIT_FOR_INIT))?;
        for event in events.iter() {
            // check if the event token in PARENT
            // note that this does not assign anything to PARENT, but instead compares PARENT and event.token()
            // check http://patshaughnessy.net/2018/1/18/learning-rust-if-let-vs--match for a bit more detailed explanation
            if let CHILD = event.token() {
                // read message from the init process
                let mut buf = [0; 1];
                match receiver.read_exact(&mut buf) {
                    // This error simply means that there are no more incoming connections waiting to be accepted at this point.
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => (),
                    Err(e) => bail!(
                        "Failed to receive a message from the child process. {:?}",
                        e
                    ),
                    _ => (),
                }
                match Message::from(u8::from_be_bytes(buf)) {
                    Message::InitReady => return Ok(()),
                    msg => bail!("receive unexpected message {:?} in child process", msg),
                }
            } else {
                unreachable!()
            }
        }
        // should not reach here, as there should be a ready event from init within WAIT_FOR_INIT duration
        unreachable!(
            "No message received from init process within {} seconds",
            WAIT_FOR_INIT.as_secs()
        );
    }
}
