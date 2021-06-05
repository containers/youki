use std::io::Read;

use super::{MAX_EVENTS, WAIT_FOR_CHILD};
use crate::process::message::Message;
use anyhow::{bail, Result};
use mio::unix::pipe;
use mio::unix::pipe::{Receiver, Sender};
use mio::{Events, Interest, Poll, Token};

// Token is used to identify which socket generated an event
const PARENT: Token = Token(0);

/// Contains receiving end of pipe to child process and a poller for that.
pub struct ParentProcess {
    receiver: Receiver,
    poll: Poll,
}

// Poll is used to register and listen for various events
// by registering it with an event source such as receiving end of a pipe
impl ParentProcess {
    /// Create new Parent process structure
    pub fn new() -> Result<(Self, Sender)> {
        // create a new pipe
        let (sender, mut receiver) = pipe::new()?;
        // create a new poll, and register the receiving end of pipe to it
        // This will poll for the read events, so when data is written to sending end of the pipe,
        // the receiving end will be readable and poll wil notify
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, PARENT, Interest::READABLE)?;
        Ok((Self { receiver, poll }, sender))
    }

    /// Waits for associated child process to send ready message
    /// and return the pid of init process which is forked by child process
    pub fn wait_for_child_ready(&mut self) -> Result<i32> {
        // Create collection with capacity to store up to 128 events
        let mut events = Events::with_capacity(MAX_EVENTS);

        // poll the receiving end of pipe created for 5 seconds for an event
        self.poll.poll(&mut events, Some(WAIT_FOR_CHILD))?;
        for event in events.iter() {
            // check if the event token in PARENT
            // note that this does not assign anything to PARENT, but instead compares PARENT and event.token()
            // check http://patshaughnessy.net/2018/1/18/learning-rust-if-let-vs--match for a bit more detailed explanation
            if let PARENT = event.token() {
                // read data from pipe
                let mut buf = [0; 1];
                self.receiver.read_exact(&mut buf)?;
                // convert to Message wrapper
                match Message::from(u8::from_be_bytes(buf)) {
                    Message::ChildReady => {
                        // read pid of init process forked by child, 4 bytes as the type is i32
                        let mut buf = [0; 4];
                        self.receiver.read_exact(&mut buf)?;
                        return Ok(i32::from_be_bytes(buf));
                    }
                    msg => bail!("receive unexpected message {:?} in parent process", msg),
                }
            } else {
                // as the poll is registered with only parent token
                unreachable!()
            }
        }
        // should not reach here, as there should be a ready event from child within 5 seconds
        unreachable!(
            "No message received from child process within {} seconds",
            WAIT_FOR_CHILD.as_secs()
        );
    }
}
