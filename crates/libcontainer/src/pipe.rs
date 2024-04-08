use std::io;
use std::mem;
use std::os::unix::io::RawFd;

use libc;
use libc::{c_void, size_t};
use nix::fcntl::OFlag;
use nix::unistd::pipe2;

/// A pipe used to communicate with subprocess
#[derive(Debug)]
pub struct Pipe(RawFd, RawFd);

/// A reading end of `Pipe` object after `Pipe::split`
#[derive(Debug)]
pub struct PipeReader(RawFd);

/// A writing end of `Pipe` object after `Pipe::split`
#[derive(Debug)]
pub struct PipeWriter(RawFd);

#[derive(Debug)]
pub enum PipeHolder {
    Reader(PipeReader),
    Writer(PipeWriter),
}

#[derive(Debug, thiserror::Error)]
pub enum PipeError {
    #[error("failed to create pipe: {0}")]
    Create(nix::Error),
    #[error("failed to open fd: {0}")]
    Open(nix::Error),
    #[error("failed to dup fd: {0}")]
    Dup(nix::Error),
}

impl Pipe {
    pub fn new() -> Result<Pipe, PipeError> {
        let (rd, wr) = pipe2(OFlag::O_CLOEXEC).map_err(PipeError::Create)?;
        Ok(Pipe(rd, wr))
    }
    pub fn split(self) -> (PipeReader, PipeWriter) {
        let Pipe(rd, wr) = self;
        mem::forget(self);
        (PipeReader(rd), PipeWriter(wr))
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let Pipe(x, y) = *self;
        unsafe {
            libc::close(x);
            libc::close(y);
        }
    }
}

impl PipeReader {
    /// Extract file descriptor from pipe reader without closing
    // TODO(tailhook) implement IntoRawFd here
    pub fn into_fd(self) -> RawFd {
        let PipeReader(fd) = self;
        mem::forget(self);
        fd
    }
}

impl PipeWriter {
    /// Extract file descriptor from pipe reader without closing
    // TODO(tailhook) implement IntoRawFd her
    pub fn into_fd(self) -> RawFd {
        let PipeWriter(fd) = self;
        mem::forget(self);
        fd
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

impl io::Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let ret =
            unsafe { libc::read(self.0, buf.as_mut_ptr() as *mut c_void, buf.len() as size_t) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ret as usize)
    }
}

impl io::Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let ret =
            unsafe { libc::write(self.0, buf.as_ptr() as *const c_void, buf.len() as size_t) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ret as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
