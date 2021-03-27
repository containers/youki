use std::io::Write;

use anyhow::Result;
use mio::unix::pipe::Sender;

use crate::process::message::Message;
pub struct InitProcess {
    sender_for_child: Sender,
}

impl InitProcess {
    pub fn new(sender_for_child: Sender) -> Self {
        Self { sender_for_child }
    }

    pub fn ready(&mut self) -> Result<()> {
        log::debug!(
            "init send to child {:?}",
            (Message::InitReady as u8).to_be_bytes()
        );
        self.write_message_for_child(Message::InitReady)?;
        Ok(())
    }

    fn write_message_for_child(&mut self, msg: Message) -> Result<()> {
        self.sender_for_child
            .write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }
}
