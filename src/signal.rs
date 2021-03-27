use anyhow::{bail, Result};
use nix::sys::signal::Signal;

pub fn from_str(signal: &str) -> Result<Signal> {
    use Signal::*;
    Ok(match signal.to_ascii_uppercase().as_str() {
        "1" | "HUP" | "SIGHUP" => Signal::SIGHUP,
        "2" | "INT" | "SIGINT" => Signal::SIGINT,
        "3" | "QUIT" | "SIGQUIT" => Signal::SIGQUIT,
        "4" | "ILL" | "SIGILL" => Signal::SIGILL,
        "5" | "BUS" | "SIGBUS" => Signal::SIGBUS,
        "6" | "ABRT" | "IOT" | "SIGABRT" | "SIGIOT" => Signal::SIGABRT,
        "7" | "TRAP" | "SIGTRAP" => Signal::SIGTRAP,
        "8" | "FPE" | "SIGFPE" => Signal::SIGFPE,
        "9" | "KILL" | "SIGKILL" => Signal::SIGKILL,
        "10" | "USR1" | "SIGUSR1" => Signal::SIGUSR1,
        "11" | "SEGV" | "SIGSEGV" => SIGSEGV,
        "12" | "USR2" | "SIGUSR2" => SIGUSR2,
        "13" | "PIPE" | "SIGPIPE" => SIGPIPE,
        "14" | "ALRM" | "SIGALRM" => SIGALRM,
        "15" | "TERM" | "SIGTERM" => SIGTERM,
        "16" | "STKFLT" | "SIGSTKFLT" => SIGSTKFLT,
        "17" | "CHLD" | "SIGCHLD" => SIGCHLD,
        "18" | "CONT" | "SIGCONT" => SIGCONT,
        "19" | "STOP" | "SIGSTOP" => SIGSTOP,
        "20" | "TSTP" | "SIGTSTP" => SIGTSTP,
        "21" | "TTIN" | "SIGTTIN" => SIGTTIN,
        "22" | "TTOU" | "SIGTTOU" => SIGTTOU,
        "23" | "URG" | "SIGURG" => SIGURG,
        "24" | "XCPU" | "SIGXCPU" => SIGXCPU,
        "25" | "XFSZ" | "SIGXFSZ" => SIGXFSZ,
        "26" | "VTALRM" | "SIGVTALRM" => SIGVTALRM,
        "27" | "PROF" | "SIGPROF" => SIGPROF,
        "28" | "WINCH" | "SIGWINCH" => SIGWINCH,
        "29" | "IO" | "SIGIO" => SIGIO,
        "30" | "PWR" | "SIGPWR" => SIGPWR,
        "31" | "SYS" | "SIGSYS" => SIGSYS,
        _ => bail! {"{} is not a valid signal", signal},
    })
}
