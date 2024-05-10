// Code mostly copied from the `unshare` crate.

use std::io;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};

use nix::fcntl::{fcntl, FcntlArg};

/// An enumeration that is used to configure stdio file descritors
///
/// The enumeration members might be non-stable, it's better to use
/// one of the constructors to create an instance
#[derive(Default)]
pub enum Stdio {
    /// This fd will be inherited from the parent application
    #[default]
    Inherit,
    /// This fd will use pipe to/from the appliation
    Pipe,
    /// This fd will open /dev/null in read or write mode
    Null,
    /// This is fd passed by application (and closed by `unshare`)
    Fd(Closing),
}

/// An enumeration that is used to configure non-stdio file descriptors. It
/// differs from stdio one because we must differentiate from readable and
/// writable file descriptors for things open by the library
///
/// The enumeration members might be non-stable, it's better to use
/// one of the constructors to create an instance
pub enum Fd {
    /// This fd is a reading end of a pipe
    ReadPipe,
    /// This fd is a writing end of a pipe
    WritePipe,
    /// This fd is inherited from parent (current) process
    Inherit,
    /// This fd is redirected from `/dev/null`
    ReadNull,
    /// This fd is redirected to `/dev/null`
    WriteNull,
    /// This is fd passed by application (and closed by `unshare`)
    Fd(Closing),
}

pub struct Closing(RawFd);

pub fn dup_file_cloexec<F: AsRawFd>(file: &F) -> io::Result<Closing> {
    match fcntl(file.as_raw_fd(), FcntlArg::F_DUPFD_CLOEXEC(3)) {
        Ok(fd) => Ok(Closing::new(fd)),
        Err(errno) => Err(io::Error::from_raw_os_error(errno as i32)),
    }
}

impl Stdio {
    /// Pipe is created for child process
    pub fn piped() -> Stdio {
        Stdio::Pipe
    }
    /// The child inherits file descriptor from the parent process
    pub fn inherit() -> Stdio {
        Stdio::Inherit
    }
    /// Stream is attached to `/dev/null`
    pub fn null() -> Stdio {
        Stdio::Null
    }
    /// Converts stdio definition to file descriptor definition
    /// (mostly needed internally)
    pub fn to_fd(self, write: bool) -> Fd {
        match (self, write) {
            (Stdio::Fd(x), _) => Fd::Fd(x),
            (Stdio::Pipe, false) => Fd::ReadPipe,
            (Stdio::Pipe, true) => Fd::WritePipe,
            (Stdio::Inherit, _) => Fd::Inherit,
            (Stdio::Null, false) => Fd::ReadNull,
            (Stdio::Null, true) => Fd::WriteNull,
        }
    }
    /// A simpler helper method for `from_raw_fd`, that does dup of file
    /// descriptor, so is actually safe to use (but can fail)
    pub fn dup_file<F: AsRawFd>(file: &F) -> io::Result<Stdio> {
        dup_file_cloexec(file).map(Stdio::Fd)
    }
    /// A simpler helper method for `from_raw_fd`, that consumes file
    ///
    /// Note: we assume that file descriptor **already has** the `CLOEXEC`
    /// flag. This is by default for all files opened by rust.
    pub fn from_file<F: IntoRawFd>(file: F) -> Stdio {
        Stdio::Fd(Closing(file.into_raw_fd()))
    }
}

impl Fd {
    /// Create a pipe so that child can read from it
    pub fn piped_read() -> Fd {
        Fd::ReadPipe
    }
    /// Create a pipe so that child can write to it
    pub fn piped_write() -> Fd {
        Fd::WritePipe
    }
    /// Inherit the child descriptor from parent
    ///
    /// Not very useful for custom file descriptors better use `from_file()`
    pub fn inherit() -> Fd {
        Fd::Inherit
    }
    /// Create a readable pipe that always has end of file condition
    pub fn read_null() -> Fd {
        Fd::ReadNull
    }
    /// Create a writable pipe that ignores all the input
    pub fn write_null() -> Fd {
        Fd::WriteNull
    }
    /// A simpler helper method for `from_raw_fd`, that does dup of file
    /// descriptor, so is actually safe to use (but can fail)
    pub fn dup_file<F: AsRawFd>(file: &F) -> io::Result<Fd> {
        dup_file_cloexec(file).map(Fd::Fd)
    }
    /// A simpler helper method for `from_raw_fd`, that consumes file
    pub fn from_file<F: IntoRawFd>(file: F) -> Fd {
        Fd::Fd(Closing(file.into_raw_fd()))
    }
}

impl Closing {
    pub fn new(fd: RawFd) -> Closing {
        Closing(fd)
    }
}

impl AsRawFd for Closing {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for Closing {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}
