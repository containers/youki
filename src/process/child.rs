use std::io::Write;
use std::{io::Read, time::Duration};

use anyhow::{bail, Result};
use mio::unix::pipe;
use mio::unix::pipe::Receiver;
use mio::unix::pipe::Sender;
use mio::{Events, Interest, Poll, Token};
use nix::unistd::Pid;

use crate::process::message::Message;

const CHILD: Token = Token(1);
pub struct ChildProcess {
    sender_for_parent: Sender,
    receiver: Option<Receiver>,
    poll: Option<Poll>,
}

impl ChildProcess {
    pub fn new(sender_for_parent: Sender) -> Result<Self> {
        Ok(Self {
            sender_for_parent,
            receiver: None,
            poll: None,
        })
    }

    pub fn setup_uds(&mut self) -> Result<Sender> {
        let (sender, mut receiver) = pipe::new()?;
        let poll = Poll::new()?;
        poll.registry()
            .register(&mut receiver, CHILD, Interest::READABLE)?;
        self.receiver = Some(receiver);
        self.poll = Some(poll);
        Ok(sender)
    }

    pub fn ready(&mut self, init_pid: Pid) -> Result<()> {
        log::debug!(
            "child send to parent {:?}",
            (Message::ChildReady as u8).to_be_bytes()
        );
        self.write_message_for_parent(Message::ChildReady)?;
        self.sender_for_parent
            .write_all(&(init_pid.as_raw()).to_be_bytes())?;
        Ok(())
    }

    fn write_message_for_parent(&mut self, msg: Message) -> Result<()> {
        self.sender_for_parent
            .write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }

    pub fn wait_for_init_ready(&mut self) -> Result<()> {
        let receiver = self
            .receiver
            .as_mut()
            .expect("Complete the setup of uds in advance.");
        let poll = self
            .poll
            .as_mut()
            .expect("Complete the setup of uds in advance.");

        let mut events = Events::with_capacity(128);
        poll.poll(&mut events, Some(Duration::from_millis(1000)))?;
        for event in events.iter() {
            if let CHILD = event.token() {
                let mut buf = [0; 1];
                receiver.read_exact(&mut buf)?;
                match Message::from(u8::from_be_bytes(buf)) {
                    Message::InitReady => return Ok(()),
                    msg => bail!("receive unexpected message {:?} in child process", msg),
                }
            } else {
                unreachable!()
            }
        }
        bail!("unexpected message.")
    }
}
