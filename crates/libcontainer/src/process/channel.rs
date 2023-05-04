use crate::process::message::Message;
use nix::{
    sys::socket::{self, UnixAddr},
    unistd::{self, Pid},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{IoSlice, IoSliceMut},
    marker::PhantomData,
    os::unix::prelude::{AsRawFd, RawFd},
};

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("received unexpected message: {received:?}, expected: {expected:?}")]
    UnexpectedMessage {
        expected: Message,
        received: Message,
    },
    #[error("failed to receive. {msg:?}. {source:?}")]
    ReceiveError {
        msg: String,
        #[source]
        source: BaseChannelError,
    },
    #[error(transparent)]
    BaseChannelError(#[from] BaseChannelError),
    #[error("missing fds from seccomp request")]
    MissingSeccompFds,
    #[error("exec process failed with error {0}")]
    ExecError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum BaseChannelError {
    #[error("failed unix syscalls")]
    UnixError(#[from] nix::Error),
    #[error("failed serde serialization")]
    SerdeError(#[from] serde_json::Error),
    #[error("channel connection broken")]
    BrokenChannel,
}

/// Channel Design
///
/// Each of the main, intermediate, and init process will have a uni-directional
/// channel, a sender and a receiver. Each process will hold the receiver and
/// listen message on it. Each sender is shared between each process to send
/// message to the corresponding receiver. For example, main_sender and
/// main_receiver is used for the main process. The main process will use
/// receiver to receive all message sent to the main process. The other
/// processes will share the main_sender and use it to send message to the main
/// process.

pub fn main_channel() -> Result<(MainSender, MainReceiver), ChannelError> {
    let (sender, receiver) = channel::<Message>()?;
    Ok((MainSender { sender }, MainReceiver { receiver }))
}

pub struct MainSender {
    sender: Sender<Message>,
}

impl MainSender {
    // requests the Main to write the id mappings for the intermediate process
    // this needs to be done from the parent see https://man7.org/linux/man-pages/man7/user_namespaces.7.html
    pub fn identifier_mapping_request(&mut self) -> Result<(), ChannelError> {
        log::debug!("send identifier mapping request");
        self.sender.send(Message::WriteMapping)?;

        Ok(())
    }

    pub fn seccomp_notify_request(&mut self, fd: RawFd) -> Result<(), ChannelError> {
        self.sender
            .send_fds(Message::SeccompNotify, &[fd.as_raw_fd()])?;

        Ok(())
    }

    pub fn intermediate_ready(&mut self, pid: Pid) -> Result<(), ChannelError> {
        // Send over the IntermediateReady follow by the pid.
        log::debug!("sending init pid ({:?})", pid);
        self.sender.send(Message::IntermediateReady(pid.as_raw()))?;

        Ok(())
    }

    pub fn init_ready(&mut self) -> Result<(), ChannelError> {
        self.sender.send(Message::InitReady)?;

        Ok(())
    }

    pub fn exec_failed(&mut self, err: String) -> Result<(), ChannelError> {
        self.sender.send(Message::ExecFailed(err))?;
        Ok(())
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.sender.close()?;

        Ok(())
    }
}

pub struct MainReceiver {
    receiver: Receiver<Message>,
}

impl MainReceiver {
    /// Waits for associated intermediate process to send ready message
    /// and return the pid of init process which is forked by intermediate process
    pub fn wait_for_intermediate_ready(&mut self) -> Result<Pid, ChannelError> {
        let msg = self
            .receiver
            .recv()
            .map_err(|err| ChannelError::ReceiveError {
                msg: "waiting for intermediate process".to_string(),
                source: err,
            })?;

        match msg {
            Message::IntermediateReady(pid) => Ok(Pid::from_raw(pid)),
            Message::ExecFailed(err) => Err(ChannelError::ExecError(err)),
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::IntermediateReady(0),
                received: msg,
            }),
        }
    }

    pub fn wait_for_mapping_request(&mut self) -> Result<(), ChannelError> {
        let msg = self
            .receiver
            .recv()
            .map_err(|err| ChannelError::ReceiveError {
                msg: "waiting for mapping request".to_string(),
                source: err,
            })?;
        match msg {
            Message::WriteMapping => Ok(()),
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::WriteMapping,
                received: msg,
            }),
        }
    }

    pub fn wait_for_seccomp_request(&mut self) -> Result<i32, ChannelError> {
        let (msg, fds) = self.receiver.recv_with_fds::<[RawFd; 1]>().map_err(|err| {
            ChannelError::ReceiveError {
                msg: "waiting for seccomp request".to_string(),
                source: err,
            }
        })?;

        match msg {
            Message::SeccompNotify => {
                let fd = match fds {
                    Some(fds) => {
                        if fds.is_empty() {
                            Err(ChannelError::MissingSeccompFds)
                        } else {
                            Ok(fds[0])
                        }
                    }
                    None => Err(ChannelError::MissingSeccompFds),
                }?;
                Ok(fd)
            }
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::SeccompNotify,
                received: msg,
            }),
        }
    }

    /// Waits for associated init process to send ready message
    /// and return the pid of init process which is forked by init process
    pub fn wait_for_init_ready(&mut self) -> Result<(), ChannelError> {
        let msg = self
            .receiver
            .recv()
            .map_err(|err| ChannelError::ReceiveError {
                msg: "waiting for init ready".to_string(),
                source: err,
            })?;
        match msg {
            Message::InitReady => Ok(()),
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::InitReady,
                received: msg,
            }),
        }
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.receiver.close()?;

        Ok(())
    }
}

pub fn intermediate_channel() -> Result<(IntermediateSender, IntermediateReceiver), ChannelError> {
    let (sender, receiver) = channel::<Message>()?;
    Ok((
        IntermediateSender { sender },
        IntermediateReceiver { receiver },
    ))
}

pub struct IntermediateSender {
    sender: Sender<Message>,
}

impl IntermediateSender {
    pub fn mapping_written(&mut self) -> Result<(), ChannelError> {
        log::debug!("identifier mapping written");
        self.sender.send(Message::MappingWritten)?;

        Ok(())
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.sender.close()?;

        Ok(())
    }
}

pub struct IntermediateReceiver {
    receiver: Receiver<Message>,
}

impl IntermediateReceiver {
    // wait until the parent process has finished writing the id mappings
    pub fn wait_for_mapping_ack(&mut self) -> Result<(), ChannelError> {
        log::debug!("waiting for mapping ack");
        let msg = self
            .receiver
            .recv()
            .map_err(|err| ChannelError::ReceiveError {
                msg: "waiting for mapping ack".to_string(),
                source: err,
            })?;
        match msg {
            Message::MappingWritten => Ok(()),
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::MappingWritten,
                received: msg,
            }),
        }
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.receiver.close()?;

        Ok(())
    }
}

pub fn init_channel() -> Result<(InitSender, InitReceiver), ChannelError> {
    let (sender, receiver) = channel::<Message>()?;
    Ok((InitSender { sender }, InitReceiver { receiver }))
}

pub struct InitSender {
    sender: Sender<Message>,
}

impl InitSender {
    pub fn seccomp_notify_done(&mut self) -> Result<(), ChannelError> {
        self.sender.send(Message::SeccompNotifyDone)?;

        Ok(())
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.sender.close()?;

        Ok(())
    }
}

pub struct InitReceiver {
    receiver: Receiver<Message>,
}

impl InitReceiver {
    pub fn wait_for_seccomp_request_done(&mut self) -> Result<(), ChannelError> {
        let msg = self
            .receiver
            .recv()
            .map_err(|err| ChannelError::ReceiveError {
                msg: "waiting for seccomp request".to_string(),
                source: err,
            })?;

        match msg {
            Message::SeccompNotifyDone => Ok(()),
            msg => Err(ChannelError::UnexpectedMessage {
                expected: Message::SeccompNotifyDone,
                received: msg,
            }),
        }
    }

    pub fn close(&self) -> Result<(), ChannelError> {
        self.receiver.close()?;

        Ok(())
    }
}

pub struct Receiver<T> {
    receiver: RawFd,
    phantom: PhantomData<T>,
}

pub struct Sender<T> {
    sender: RawFd,
    phantom: PhantomData<T>,
}

impl<T> Sender<T>
where
    T: Serialize,
{
    fn send_iovec(
        &mut self,
        iov: &[IoSlice],
        fds: Option<&[RawFd]>,
    ) -> Result<usize, BaseChannelError> {
        let cmsgs = if let Some(fds) = fds {
            vec![socket::ControlMessage::ScmRights(fds)]
        } else {
            vec![]
        };
        socket::sendmsg::<UnixAddr>(self.sender, iov, &cmsgs, socket::MsgFlags::empty(), None)
            .map_err(|e| e.into())
    }

    fn send_slice_with_len(
        &mut self,
        data: &[u8],
        fds: Option<&[RawFd]>,
    ) -> Result<usize, BaseChannelError> {
        let len = data.len() as u64;
        // Here we prefix the length of the data onto the serialized data.
        let iov = [
            IoSlice::new(unsafe {
                std::slice::from_raw_parts(
                    (&len as *const u64) as *const u8,
                    std::mem::size_of::<u64>(),
                )
            }),
            IoSlice::new(data),
        ];
        self.send_iovec(&iov[..], fds)
    }

    pub fn send(&mut self, object: T) -> Result<(), BaseChannelError> {
        let payload = serde_json::to_vec(&object)?;
        self.send_slice_with_len(&payload, None)?;

        Ok(())
    }

    pub fn send_fds(&mut self, object: T, fds: &[RawFd]) -> Result<(), BaseChannelError> {
        let payload = serde_json::to_vec(&object)?;
        self.send_slice_with_len(&payload, Some(fds))?;

        Ok(())
    }

    pub fn close(&self) -> Result<(), BaseChannelError> {
        Ok(unistd::close(self.sender)?)
    }
}

impl<T> Receiver<T>
where
    T: serde::de::DeserializeOwned,
{
    fn peek_size_iovec(&mut self) -> Result<u64, BaseChannelError> {
        let mut len: u64 = 0;
        let mut iov = [IoSliceMut::new(unsafe {
            std::slice::from_raw_parts_mut(
                (&mut len as *mut u64) as *mut u8,
                std::mem::size_of::<u64>(),
            )
        })];
        let _ =
            socket::recvmsg::<UnixAddr>(self.receiver, &mut iov, None, socket::MsgFlags::MSG_PEEK)?;
        match len {
            0 => Err(BaseChannelError::BrokenChannel),
            _ => Ok(len),
        }
    }

    fn recv_into_iovec<F>(
        &mut self,
        iov: &mut [IoSliceMut],
    ) -> Result<(usize, Option<F>), BaseChannelError>
    where
        F: Default + AsMut<[RawFd]>,
    {
        let mut cmsgspace = nix::cmsg_space!(F);
        let msg = socket::recvmsg::<UnixAddr>(
            self.receiver,
            iov,
            Some(&mut cmsgspace),
            socket::MsgFlags::MSG_CMSG_CLOEXEC,
        )?;

        // Sending multiple SCM_RIGHTS message will led to platform dependent
        // behavior, with some system choose to return EINVAL when sending or
        // silently only process the first msg or send all of it. Here we assume
        // there is only one SCM_RIGHTS message and will only process the first
        // message.
        let fds: Option<F> = msg
            .cmsgs()
            .find_map(|cmsg| {
                if let socket::ControlMessageOwned::ScmRights(fds) = cmsg {
                    Some(fds)
                } else {
                    None
                }
            })
            .map(|fds| {
                let mut fds_array: F = Default::default();
                <F as AsMut<[RawFd]>>::as_mut(&mut fds_array).clone_from_slice(&fds);
                fds_array
            });

        Ok((msg.bytes, fds))
    }

    fn recv_into_buf_with_len<F>(&mut self) -> Result<(Vec<u8>, Option<F>), BaseChannelError>
    where
        F: Default + AsMut<[RawFd]>,
    {
        let msg_len = self.peek_size_iovec()?;
        let mut len: u64 = 0;
        let mut buf = vec![0u8; msg_len as usize];
        let (bytes, fds) = {
            let mut iov = [
                IoSliceMut::new(unsafe {
                    std::slice::from_raw_parts_mut(
                        (&mut len as *mut u64) as *mut u8,
                        std::mem::size_of::<u64>(),
                    )
                }),
                IoSliceMut::new(&mut buf),
            ];
            self.recv_into_iovec(&mut iov)?
        };

        match bytes {
            0 => Err(BaseChannelError::BrokenChannel),
            _ => Ok((buf, fds)),
        }
    }

    // Recv the next message of type T.
    pub fn recv(&mut self) -> Result<T, BaseChannelError> {
        let (buf, _) = self.recv_into_buf_with_len::<[RawFd; 0]>()?;
        Ok(serde_json::from_slice(&buf[..])?)
    }

    // Works similar to `recv`, but will look for fds sent by SCM_RIGHTS
    // message.  We use F as as `[RawFd; n]`, where `n` is the number of
    // descriptors you want to receive.
    pub fn recv_with_fds<F>(&mut self) -> Result<(T, Option<F>), BaseChannelError>
    where
        F: Default + AsMut<[RawFd]>,
    {
        let (buf, fds) = self.recv_into_buf_with_len::<F>()?;
        Ok((serde_json::from_slice(&buf[..])?, fds))
    }

    pub fn close(&self) -> Result<(), BaseChannelError> {
        Ok(unistd::close(self.receiver)?)
    }
}

pub fn channel<T>() -> Result<(Sender<T>, Receiver<T>), BaseChannelError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let (os_sender, os_receiver) = unix_channel()?;
    let receiver = Receiver {
        receiver: os_receiver,
        phantom: PhantomData,
    };
    let sender = Sender {
        sender: os_sender,
        phantom: PhantomData,
    };
    Ok((sender, receiver))
}

// Use socketpair as the underlying pipe.
fn unix_channel() -> Result<(RawFd, RawFd), BaseChannelError> {
    Ok(socket::socketpair(
        socket::AddressFamily::Unix,
        socket::SockType::SeqPacket,
        None,
        socket::SockFlag::SOCK_CLOEXEC,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    use nix::sys::wait;
    use nix::unistd;
    use serial_test::serial;

    // Note: due to cargo test by default runs tests in parallel using a single
    // process, these tests should not be running in parallel with other tests.
    // Because we run tests in the same process, other tests may decide to close
    // down file descriptors or saturate the IOs in the OS.  The channel uses
    // pipe to communicate and can potentially become flaky as a result. There
    // is not much else we can do other than to run the tests in serial.

    #[test]
    #[serial]
    fn test_channel_intermadiate_ready() -> Result<()> {
        let (sender, receiver) = &mut main_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                wait::waitpid(child, None)?;
                let pid = receiver
                    .wait_for_intermediate_ready()
                    .with_context(|| "Failed to wait for intermadiate ready")?;
                receiver.close()?;
                assert_eq!(pid, child);
            }
            unistd::ForkResult::Child => {
                let pid = unistd::getpid();
                sender.intermediate_ready(pid)?;
                sender.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    #[serial]
    fn test_channel_id_mapping_request() -> Result<()> {
        let (sender, receiver) = &mut main_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                wait::waitpid(child, None)?;
                receiver.wait_for_mapping_request()?;
                receiver.close()?;
            }
            unistd::ForkResult::Child => {
                sender
                    .identifier_mapping_request()
                    .with_context(|| "Failed to send mapping written")?;
                sender.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    #[serial]
    fn test_channel_id_mapping_ack() -> Result<()> {
        let (sender, receiver) = &mut intermediate_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                wait::waitpid(child, None)?;
                receiver.wait_for_mapping_ack()?;
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
    #[serial]
    fn test_channel_init_ready() -> Result<()> {
        let (sender, receiver) = &mut main_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                wait::waitpid(child, None)?;
                receiver.wait_for_init_ready()?;
                receiver.close()?;
            }
            unistd::ForkResult::Child => {
                sender
                    .init_ready()
                    .with_context(|| "Failed to send init ready")?;
                sender.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    #[serial]
    fn test_channel_main_graceful_exit() -> Result<()> {
        let (sender, receiver) = &mut main_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                sender.close().context("failed to close sender")?;
                // The child process will exit without send the intermediate ready
                // message. This should cause the wait_for_intermediate_ready to error
                // out, instead of keep blocking.
                let ret = receiver.wait_for_intermediate_ready();
                assert!(ret.is_err());
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                receiver.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    #[test]
    #[serial]
    fn test_channel_intermediate_graceful_exit() -> Result<()> {
        let (sender, receiver) = &mut main_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                sender.close().context("failed to close sender")?;
                // The child process will exit without send the init ready
                // message. This should cause the wait_for_init_ready to error
                // out, instead of keep blocking.
                let ret = receiver.wait_for_init_ready();
                assert!(ret.is_err());
                wait::waitpid(child, None)?;
            }
            unistd::ForkResult::Child => {
                receiver.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}
