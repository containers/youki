use anyhow::Result;
use mio::unix::pipe;
use mio::unix::pipe::Receiver;
use mio::unix::pipe::Sender;
use mio::{Interest, Poll, Token};

use crate::notify_socket::NotifyListener;

use super::parent::ParentChannel;

// Token is used to identify which socket generated an event
const CHILD: Token = Token(1);

/// Contains sending end of pipe for parent process, receiving end of pipe
/// for the init process and poller for that
pub struct ChildProcess {
    parent_channel: ParentChannel,
    receiver: Option<Receiver>,
    poll: Option<Poll>,
    notify: NotifyListener,
}

// Note : The original youki process first forks into 'parent' (P) and 'child' (C1) process
// of which this represents the child (C1) process. The C1 then again forks into parent process which is C1,
// and Child (C2) process. C2 is called as init process as it will run the command of the container. But form
// a process point of view, init process is child of child process, which is child of original youki process.
impl ChildProcess {
    /// create a new Child process structure
    pub fn new(parent_channel: ParentChannel, notify: NotifyListener) -> Result<Self> {
        Ok(Self {
            parent_channel,
            receiver: None,
            poll: None,
            notify,
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
    pub fn notify_parent(&mut self) -> Result<()> {
        self.parent_channel.send_child_ready()?;
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

    pub fn wait_for_container_start(&mut self) -> Result<()> {
        self.notify.wait_for_container_start()
    }
}
