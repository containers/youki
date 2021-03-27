use std::{io::Read, time::Duration};

use anyhow::{bail, Result};
use mio::unix::pipe;
use mio::unix::pipe::{Receiver, Sender};
use mio::{Events, Interest, Poll, Token};

use crate::process::message::Message;

const PARENT: Token = Token(0);
pub struct ParentProcess {
    receiver: Receiver,
    poll: Poll,
}

impl ParentProcess {
    pub fn new() -> Result<(Self, Sender)> {
        let (sender, mut receiver) = pipe::new()?;
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, PARENT, Interest::READABLE)?;
        Ok((Self { receiver, poll }, sender))
    }

    pub fn wait_for_child_ready(&mut self) -> Result<i32> {
        let mut events = Events::with_capacity(128);
        self.poll
            .poll(&mut events, Some(Duration::from_millis(1000)))?;
        for event in events.iter() {
            if let PARENT = event.token() {
                let mut buf = [0; 1];
                self.receiver.read_exact(&mut buf)?;
                match Message::from(u8::from_be_bytes(buf)) {
                    Message::ChildReady => {
                        let mut buf = [0; 4];
                        self.receiver.read_exact(&mut buf)?;
                        return Ok(i32::from_be_bytes(buf));
                    }
                    msg => bail!("receive unexpected message {:?} in parent process", msg),
                }
            } else {
                unreachable!()
            }
        }
        bail!("unexpected message.")
    }
}
