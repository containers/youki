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

trait SenderExt {
    fn write_message(&mut self, msg: Message) -> Result<()>;
}

impl SenderExt for Sender {
    #[inline]
    fn write_message(&mut self, msg: Message) -> Result<()> {
        let bytes = (msg as u8).to_be_bytes();
        self.write_all(&bytes)
            .with_context(|| format!("Failed to write message {:?} to the pipe", bytes))?;
        Ok(())
    }
}

pub fn main_to_intermediate() -> Result<(SenderMainToIntermediate, ReceiverFromMain)> {
    let (sender, receiver) = new_pipe()?;
    Ok((
        SenderMainToIntermediate { sender },
        ReceiverFromMain { receiver },
    ))
}

pub struct SenderMainToIntermediate {
    sender: Sender,
}

impl SenderMainToIntermediate {
    pub fn mapping_written(&mut self) -> Result<()> {
        log::debug!("identifier mapping written");
        self.sender
            .write_all(&(Message::MappingWritten as u8).to_be_bytes())?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.sender.as_raw_fd())?;
        Ok(())
    }
}

pub struct ReceiverFromMain {
    receiver: Receiver,
}

impl ReceiverFromMain {
    // wait until the parent process has finished writing the id mappings
    pub fn wait_for_mapping_ack(&mut self) -> Result<()> {
        log::debug!("waiting for mapping ack");
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the main process.")?;

        match Message::from(u8::from_be_bytes(buf)) {
            Message::MappingWritten => Ok(()),
            msg => bail!(
                "receive unexpected message {:?} in waiting for mapping ack",
                msg
            ),
        }
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.receiver.as_raw_fd())?;
        Ok(())
    }
}

pub fn intermediate_to_main() -> Result<(SenderIntermediateToMain, ReceiverFromIntermediate)> {
    let (sender, receiver) = new_pipe()?;
    Ok((
        SenderIntermediateToMain { sender },
        ReceiverFromIntermediate { receiver },
    ))
}

pub struct SenderIntermediateToMain {
    sender: Sender,
}

impl SenderIntermediateToMain {
    // requests the Main to write the id mappings for the intermediate process
    // this needs to be done from the parent see https://man7.org/linux/man-pages/man7/user_namespaces.7.html
    pub fn identifier_mapping_request(&mut self) -> Result<()> {
        log::debug!("send identifier mapping request");
        self.sender.write_message(Message::WriteMapping)?;
        Ok(())
    }

    pub fn intermediate_ready(&mut self, pid: Pid) -> Result<()> {
        // Send over the IntermediateReady follow by the pid.
        log::debug!("sending init pid ({:?})", pid);
        self.sender.write_message(Message::IntermediateReady)?;
        self.sender.write_all(&(pid.as_raw()).to_be_bytes())?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.sender.as_raw_fd())?;
        Ok(())
    }
}

pub struct ReceiverFromIntermediate {
    receiver: Receiver,
}

impl ReceiverFromIntermediate {
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

    /// Waits for associated intermediate process to send ready message
    /// and return the pid of init process which is forked by intermediate process
    pub fn wait_for_intermediate_ready(&mut self) -> Result<Pid> {
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the intermediate process.")?;

        match Message::from(u8::from_be_bytes(buf)) {
            Message::IntermediateReady => {
                log::debug!("received intermediate ready message");
                // Read the Pid which will be i32 or 4 bytes.
                let mut buf = [0; 4];
                self.receiver.read_exact(&mut buf).with_context(|| {
                    "Failed to receive a message from the intermediate process."
                })?;

                Ok(Pid::from_raw(i32::from_be_bytes(buf)))
            }
            msg => bail!(
                "receive unexpected message {:?} waiting for intermediate ready",
                msg
            ),
        }
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.receiver.as_raw_fd())?;
        Ok(())
    }
}

pub fn init_to_intermediate() -> Result<(SenderInitToIntermediate, ReceiverFromInit)> {
    let (sender, receiver) = new_pipe()?;
    Ok((
        SenderInitToIntermediate { sender },
        ReceiverFromInit { receiver },
    ))
}

pub struct SenderInitToIntermediate {
    sender: Sender,
}

impl SenderInitToIntermediate {
    pub fn init_ready(&mut self) -> Result<()> {
        self.sender.write_message(Message::InitReady)?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.sender.as_raw_fd())?;
        Ok(())
    }
}

pub struct ReceiverFromInit {
    receiver: Receiver,
}

impl ReceiverFromInit {
    /// Waits for associated init process to send ready message
    /// and return the pid of init process which is forked by init process
    pub fn wait_for_init_ready(&mut self) -> Result<()> {
        let mut buf = [0; 1];
        self.receiver
            .read_exact(&mut buf)
            .with_context(|| "Failed to receive a message from the init process.")?;

        match Message::from(u8::from_be_bytes(buf)) {
            Message::InitReady => Ok(()),
            msg => bail!(
                "receive unexpected message {:?} waiting for init ready",
                msg
            ),
        }
    }

    pub fn close(&self) -> Result<()> {
        unistd::close(self.receiver.as_raw_fd())?;
        Ok(())
    }
}

fn new_pipe() -> Result<(Sender, Receiver)> {
    let (sender, receiver) = pipe::new()?;
    // Our use case is for the process to wait for the communication to come
    // through, so we set nonblocking to false here (double negative). It is
    // expected that the waiting process will block and wait.
    receiver
        .set_nonblocking(false)
        .with_context(|| "Failed to set channel receiver to blocking")?;
    Ok((sender, receiver))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use nix::sys::wait;
    use nix::unistd;

    #[test]
    fn test_channel_intermadiate_ready() -> Result<()> {
        let (sender, receiver) = &mut intermediate_to_main()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                let pid = receiver
                    .wait_for_intermediate_ready()
                    .with_context(|| "Failed to wait for intermadiate ready")?;
                assert_eq!(pid, child);
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                let pid = unistd::getpid();
                sender
                    .intermediate_ready(pid)
                    .with_context(|| "Failed to send intermadiate ready")?;
                std::process::exit(0);
            }
        };
        Ok(())
    }

    #[test]
    fn test_channel_id_mapping_request() -> Result<()> {
        let (sender, receiver) = &mut intermediate_to_main()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                receiver
                    .wait_for_mapping_request()
                    .with_context(|| "Failed to wait for mapping ack")?;
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                sender
                    .identifier_mapping_request()
                    .with_context(|| "Failed to send mapping written")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    fn test_channel_id_mapping_ack() -> Result<()> {
        let (sender, receiver) = &mut main_to_intermediate()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                receiver
                    .wait_for_mapping_ack()
                    .with_context(|| "Failed to wait for mapping ack")?;
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                sender
                    .mapping_written()
                    .with_context(|| "Failed to send mapping written")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    fn test_channel_init_ready() -> Result<()> {
        let (sender, receiver) = &mut init_to_intermediate()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                receiver
                    .wait_for_init_ready()
                    .with_context(|| "Failed to wait for init ready")?;
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                sender
                    .init_ready()
                    .with_context(|| "Failed to send init ready")?;
                std::process::exit(0);
            }
        };
        Ok(())
    }

    #[test]
    fn test_channel_intermedaite_graceful_exit() -> Result<()> {
        let (sender, receiver) = &mut intermediate_to_main()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                sender.close().context("Failed to close sender")?;
                // The child process will exit without send the intermediate ready
                // message. This should cause the wait_for_intermediate_ready to error
                // out, instead of keep blocking.
                let ret = receiver.wait_for_intermediate_ready();
                assert!(ret.is_err());
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                receiver.close().context("Failed to close receiver")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    fn test_channel_init_graceful_exit() -> Result<()> {
        let (sender, receiver) = &mut init_to_intermediate()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                sender.close().context("Failed to close sender")?;
                // The child process will exit without send the init ready
                // message. This should cause the wait_for_init_ready to error
                // out, instead of keep blocking.
                let ret = receiver.wait_for_init_ready();
                assert!(ret.is_err());
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                receiver.close().context("Failed to close receiver")?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}
