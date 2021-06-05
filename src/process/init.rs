use std::io::Write;

use anyhow::Result;
use mio::unix::pipe::Sender;

use crate::process::message::Message;

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
