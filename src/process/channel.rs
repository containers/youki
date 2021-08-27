use crate::process::message::Message;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use mio::unix::pipe;
use mio::unix::pipe::{Receiver, Sender};
use nix::unistd;
use nix::unistd::Pid;
use std::io::Read;
use std::io::Write;
use std::os::unix::io::AsRawFd;

pub struct Channel {
    sender: Sender,
    receiver: Receiver,
}

impl Channel {
    pub fn new() -> Result<Self> {
        let (sender, receiver) = pipe::new()?;
        // Our use case is for the process to wait for the communication to come
        // through, so we set nonblocking to false here (double negative). It is
        // expected that the waiting process will block and wait.
        receiver
            .set_nonblocking(false)
            .with_context(|| "Failed to set channel receiver to blocking")?;
        Ok(Self { sender, receiver })
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
        log::debug!("waiting for mapping ack");
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the child process.")?;

        match Message::from(u8::from_be_bytes(buf)) {
            Message::MappingWritten => Ok(()),
            msg => bail!(
                "receive unexpected message {:?} in waiting for mapping ack",
                msg
            ),
        }
    }

    pub fn wait_for_mapping_request(&mut self) -> Result<()> {
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the child process.")?;

        // convert to Message wrapper
        match Message::from(u8::from_be_bytes(buf)) {
            Message::WriteMapping => Ok(()),
            msg => bail!(
                "receive unexpected message {:?} waiting for mapping request",
                msg
            ),
        }
    }

    /// Waits for associated child process to send ready message
    /// and return the pid of init process which is forked by child process
    pub fn wait_for_child_ready(&mut self) -> Result<Pid> {
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the child process.")?;

        match Message::from(u8::from_be_bytes(buf)) {
            Message::ChildReady => {
                log::debug!("received child ready message");
                // Read the Pid which will be i32 or 4 bytes.
                let mut buf = [0; 4];
                self.receiver
                    .read_exact(&mut buf)
                    .with_context(|| "Failed to receive a message from the child process.")?;

                Ok(Pid::from_raw(i32::from_be_bytes(buf)))
            }
            msg => bail!(
                "receive unexpected message {:?} waiting for child ready",
                msg
            ),
        }
    }

    pub fn close_receiver(&self) -> Result<()> {
        unistd::close(self.receiver.as_raw_fd())?;

        Ok(())
    }

    pub fn close_sender(&self) -> Result<()> {
        unistd::close(self.sender.as_raw_fd())?;

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        self.close_receiver().context("Failed to close receiver")?;
        self.close_sender().context("Failed to close sender")?;

        Ok(())
    }

    #[inline]
    fn write_message(&mut self, msg: Message) -> Result<()> {
        self.sender.write_all(&(msg as u8).to_be_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use nix::sys::wait;
    use nix::unistd;

    #[test]
    fn test_channel_child_ready() -> Result<()> {
        let ch = &mut Channel::new()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                let pid = ch
                    .wait_for_child_ready()
                    .with_context(|| "Failed to wait for child ready")?;
                assert_eq!(pid, child);
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                let pid = unistd::getpid();
                ch.send_child_ready(pid)
                    .with_context(|| "Failed to send child ready")?;
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
                ch.wait_for_mapping_ack()
                    .with_context(|| "Failed to wait for mapping ack")?;
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                ch.send_mapping_written()
                    .with_context(|| "Failed to send mapping written")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    fn test_channel_graceful_exit() -> Result<()> {
        let ch = &mut Channel::new()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                ch.close_sender().context("Failed to close sender")?;
                // The child process will exit without send the child ready
                // message. This should cause the wait_for_child_ready to error
                // out, instead of keep blocking.
                let ret = ch.wait_for_child_ready();
                assert!(ret.is_err());
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                ch.close_receiver().context("Failed to close receiver")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}
