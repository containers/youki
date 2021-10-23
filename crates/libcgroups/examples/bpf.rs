use anyhow::{bail, Result};
use clap::{Arg, SubCommand};
use std::os::unix::io::AsRawFd;
use std::path::Path;

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;

use cgroups::v2::devices::bpf;
use cgroups::v2::devices::emulator;
use cgroups::v2::devices::program;
use oci_spec::*;

const LICENSE: &'static str = &"Apache";

fn main() -> Result<()> {
    env_logger::init();

    let matches = clap::App::new("bpf")
        .version("0.1")
        .about("tools to test BPF program for cgroups v2 devices")
        .arg(
            Arg::with_name("cgroup_dir")
                .short("c")
                .value_name("CGROUP_DIR"),
        )
        .subcommand(
            SubCommand::with_name("query")
                .help("query list of BPF programs attached to cgroup dir"),
        )
        .subcommand(
            SubCommand::with_name("detach")
                .help("detach BPF program by id")
                .arg(
                    Arg::with_name("id")
                        .value_name("PROG_ID")
                        .required(true)
                        .help("ID of BPF program returned by query command"),
                ),
        )
        .subcommand(
            SubCommand::with_name("attach")
                .help("compile rules to BPF and attach to cgroup dir")
                .arg(
                    Arg::with_name("input_file")
                        .value_name("INPUT_FILE")
                        .required(true)
                        .help("File contains Vec<LinuxDeviceCgroup> in json format"),
                ),
        )
        .get_matches_safe()?;

    let cgroup_dir = matches.value_of("cgroup_dir").unwrap();

    let cgroup_fd = nix::dir::Dir::open(
        cgroup_dir,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY,
        Mode::from_bits(0o600).unwrap(),
    )?;

    match matches.subcommand() {
        ("query", Some(_)) => {
            let progs = bpf::prog_query(cgroup_fd.as_raw_fd())?;
            for prog in &progs {
                println!("prog: id={}, fd={}", prog.id, prog.fd);
            }
        }
        ("detach", Some(submatch)) => {
            let prog_id = submatch.value_of("id").unwrap().parse::<u32>()?;
            let progs = bpf::prog_query(cgroup_fd.as_raw_fd())?;
            let prog = progs.iter().find(|v| v.id == prog_id);
            if prog.is_none() {
                bail!("can't get prog fd by prog id");
            }

            bpf::prog_detach2(prog.unwrap().fd, cgroup_fd.as_raw_fd())?;
            println!("detach ok");
        }
        ("attach", Some(submatch)) => {
            let input_file = submatch.value_of("input_file").unwrap();
            let rules = parse_cgroupv1_device_rules(&input_file)?;
            let mut emulator = emulator::Emulator::with_default_allow(false);
            emulator.add_rules(&rules)?;
            let prog = program::Program::from_rules(&emulator.rules, emulator.default_allow)?;
            let prog_fd = bpf::prog_load(LICENSE, prog.bytecodes())?;
            bpf::prog_attach(prog_fd, cgroup_fd.as_raw_fd())?;
            println!("attach ok");
        }

        (_, _) => {}
    };

    Ok(())
}

fn parse_cgroupv1_device_rules<P: AsRef<Path>>(path: P) -> Result<Vec<LinuxDeviceCgroup>> {
    let content = std::fs::read_to_string(path)?;
    let devices = serde_json::from_str(&content)?;
    Ok(devices)
}
