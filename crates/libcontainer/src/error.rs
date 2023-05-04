
/// SyscallWrapperError aims to simplify error handling of syscalls in
/// libcontainer. In many occasions, we mix nix::Error and std::io::Error, which
/// makes error handling complicated.
#[derive(Debug, thiserror::Error)]
pub enum SyscallWrapperError {
    #[error(transparent)]
    Io(std::io::Error),
    #[error(transparent)]
    Nix(nix::Error),
}

impl From<std::io::Error> for SyscallWrapperError {
    fn from(err: std::io::Error) -> Self {
        SyscallWrapperError::Io(err)
    }
}

impl From<nix::Error> for SyscallWrapperError {
    fn from(err: nix::Error) -> Self {
        SyscallWrapperError::Nix(err)
    }
}