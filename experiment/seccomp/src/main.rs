use nix::unistd::mkdir;
use seccomp::{
    instruction::{self, *},
    seccomp::{NotifyFd, Seccomp},
};

use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{IntoRawFd, OwnedFd};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::slice;

use anyhow::Result;
use nix::{
    libc,
    sys::{
        socket::{
            self, ControlMessage, ControlMessageOwned, MsgFlags, SockFlag, SockType, UnixAddr,
        },
        stat::Mode,
    },
    unistd::close,
};

fn send_fd<F: AsRawFd>(sock: OwnedFd, fd: &F) -> nix::Result<()> {
    let fd = fd.as_raw_fd();
    let cmsgs = [ControlMessage::ScmRights(slice::from_ref(&fd))];

    let iov = [IoSlice::new(b"x")];

    socket::sendmsg::<()>(sock.into_raw_fd(), &iov, &cmsgs, MsgFlags::empty(), None)?;
    Ok(())
}

fn recv_fd<F: FromRawFd>(sock: RawFd) -> nix::Result<Option<F>> {
    let mut iov_buf = [];
    let mut iov = [IoSliceMut::new(&mut iov_buf)];

    let mut cmsg_buf = nix::cmsg_space!(RawFd);
    let msg = socket::recvmsg::<UnixAddr>(sock, &mut iov, Some(&mut cmsg_buf), MsgFlags::empty())?;
    match msg.cmsgs().next() {
        Some(ControlMessageOwned::ScmRights(fds)) if fds.len() > 0 => {
            let fd = unsafe { F::from_raw_fd(fds[0]) };
            Ok(Some(fd))
        }
        _ => Ok(None),
    }
}

fn handle_notifications(notify_fd: NotifyFd) -> nix::Result<()> {
    loop {
        println!("Waiting on next");
        let req = notify_fd.recv()?.notif;
        assert_eq!(req.data.nr, libc::SYS_mkdir as i32);
        println!(
            "Got notification for mkdir(2): id={}, pid={}, nr={}",
            req.id, req.pid, req.data.nr
        );
    }
}

fn main() -> Result<()> {
    let (sock_for_child, sock_for_parent) = socket::socketpair(
        socket::AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )?;

    let _ = prctl::set_no_new_privileges(true);

    let mut bpf_prog = instruction::gen_validate(&Arch::X86);
    bpf_prog.append(&mut vec![
        // A: Check if syscall is getcwd
        Instruction::stmt(BPF_LD | BPF_W | BPF_ABS, 0),
        Instruction::jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, libc::SYS_getcwd as u32), // If false, go to B
        Instruction::stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        // B: Check if syscall is mkdir and if so, return seccomp notify
        Instruction::stmt(BPF_LD | BPF_W | BPF_ABS, 0),
        Instruction::jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, libc::SYS_mkdir as u32), // If false, go to C
        Instruction::stmt(BPF_RET | BPF_K, SECCOMP_RET_USER_NOTIF),
        // C: Pass
        Instruction::stmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    ]);

    let seccomp = Seccomp { filters: bpf_prog };

    if let nix::unistd::ForkResult::Child = unsafe { nix::unistd::fork()? } {
        // nix::unistd::ForkResult::Parent { child } => match wait::waitpid(child, None)? {
        //     wait::WaitStatus::Signaled(_, signal, _) => {
        //         if signal == Signal::SIGSYS {
        //             println!("Got SIGSYS, seccomp filter applied successfully!");
        //             return Ok(());
        //         }
        //         dbg!(signal);
        //     }
        //     wait_status => {
        //         dbg!("Unexpected wait status: {:?}", wait_status);
        //     }
        // },
        std::panic::catch_unwind(|| {
            let notify_fd = seccomp.apply().unwrap();
            println!(
                "Seccomp applied successfully with notify fd: {:?}",
                notify_fd
            );
            send_fd(sock_for_child, &notify_fd).unwrap();

            if let Err(e) = mkdir("/tmp/test", Mode::S_IRUSR | Mode::S_IWUSR) {
                eprintln!("Failed to mkdir: {}", e);
            } else {
                println!("mkdir succeeded");
            }
        })
        .unwrap();

        std::process::exit(0);
    };

    let notify_fd = recv_fd::<NotifyFd>(sock_for_parent.as_raw_fd())?.unwrap();

    close(sock_for_child.as_raw_fd())?;
    close(sock_for_parent.as_raw_fd())?;

    handle_notifications(notify_fd)?;

    Ok(())
}
