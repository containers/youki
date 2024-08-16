use nix::libc;
use nix::sys::socket::GetSockOpt;
use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd};

#[derive(Debug, Copy, Clone)]
pub struct PeerSec;

// This function implements the GetSockOpt for PeerSec, retrieving the security context label
// of a socket file descriptor into a CString.
// This function utilizes nix's GetSockOpt implementation.
// https://github.com/nix-rust/nix/blob/50e4283b35f3f34e138d138fd889f7e3c424a5c2/src/sys/socket/mod.rs#L2219
impl GetSockOpt for PeerSec {
    type Val = CString;

    fn get<F: AsFd>(&self, fd: &F) -> nix::Result<Self::Val> {
        let mut len: libc::socklen_t = libc::c_int::MAX as libc::socklen_t;
        let mut buf = vec![0u8; len as usize];
        let fd_i32 = fd.as_fd().as_raw_fd();

        let ret = unsafe {
            libc::getsockopt(
                fd_i32,
                libc::SOL_SOCKET,
                libc::SO_PEERSEC,
                buf.as_mut_ptr() as *mut libc::c_void,
                &mut len,
            )
        };

        if ret == -1 {
            return Err(nix::Error::last());
        }

        buf.truncate(len as usize);
        Ok(CString::new(buf).unwrap())
    }
}
