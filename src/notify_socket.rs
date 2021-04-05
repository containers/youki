use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use anyhow::Result;
use nix::unistd::close;

pub const NOTIFY_FILE: &str = "notify.sock";

pub struct NotifyListener {
    socket: UnixListener,
}

impl NotifyListener {
    pub fn new(root: &Path) -> Result<Self> {
        let _notify_file_path = root.join(NOTIFY_FILE);
        let stream = UnixListener::bind("notify.sock")?;
        Ok(Self { socket: stream })
    }

    pub fn wait_for_container_start(&mut self) -> Result<()> {
        match self.socket.accept() {
            Ok((mut socket, _addr)) => {
                let mut response = String::new();
                socket.read_to_string(&mut response)?;
                log::debug!("receive :{}", response);
            }
            Err(e) => println!("accept function failed: {:?}", e),
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        close(self.socket.as_raw_fd())?;
        Ok(())
    }
}

pub struct NotifySocket {}

impl NotifySocket {
    pub fn new(_root: &Path) -> Result<Self> {
        Ok(Self {})
    }

    pub fn notify_container_start(&mut self) -> Result<()> {
        log::debug!("connection start");
        let mut stream = UnixStream::connect("notify.sock")?;
        stream.write_all(b"start container")?;
        log::debug!("write finish");
        Ok(())
    }

    pub fn notify_container_finish(&mut self) -> Result<()> {
        // self.socket.write_all(b"finish container")?;
        Ok(())
    }
}
