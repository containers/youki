use std::env;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use anyhow::Result;
use nix::unistd::{self, close};

pub const NOTIFY_FILE: &str = "notify.sock";

pub struct NotifyListener {
    socket: UnixListener,
}

impl NotifyListener {
    pub fn new(socket_name: &str) -> Result<Self> {
        let stream = UnixListener::bind(socket_name)?;
        Ok(Self { socket: stream })
    }

    pub fn wait_for_container_start(&mut self) -> Result<()> {
        match self.socket.accept() {
            Ok((mut socket, _addr)) => {
                let mut response = String::new();
                socket.read_to_string(&mut response)?;
                log::debug!("received: {}", response);
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

pub struct NotifySocket {
    path: PathBuf,
}

impl NotifySocket {
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Self {
            path: socket_path.into(),
        }
    }

    pub fn notify_container_start(&mut self) -> Result<()> {
        log::debug!("notify container start");
        let cwd = env::current_dir()?;
        unistd::chdir(&*self.path.parent().unwrap())?;
        let mut stream = UnixStream::connect(&self.path.file_name().unwrap())?;
        stream.write_all(b"start container")?;
        log::debug!("notify finished");
        unistd::chdir(&*cwd)?;
        Ok(())
    }

    pub fn notify_container_finish(&mut self) -> Result<()> {
        // self.socket.write_all(b"finish container")?;
        Ok(())
    }
}
