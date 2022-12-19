mod tests;
mod utils;

use clap::Parser;
use oci_spec::runtime::Spec;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(version = "0.0.1", author = "youki team")]
struct Opts {
    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Parser, Debug)]
enum SubCommand {
    #[clap(name = "readonly_paths")]
    ReadonlyPaths,

    #[clap(name = "set_host_name")]
    SetHostName,

    #[clap(name = "mounts_recursive")]
    MountsRecursive,
}

const SPEC_PATH: &str = "/config.json";

fn get_spec() -> Spec {
    let path = PathBuf::from(SPEC_PATH);
    match Spec::load(path) {
        Ok(spec) => spec,
        Err(e) => {
            eprintln!("Error in loading spec, {:?}", e);
            std::process::exit(66);
        }
    }
}

fn main() {
    let opts: Opts = Opts::parse();
    let spec = get_spec();

    match opts.command {
        SubCommand::ReadonlyPaths => tests::validate_readonly_paths(&spec),
        SubCommand::SetHostName => tests::validate_hostname(&spec),
        SubCommand::MountsRecursive => tests::validate_mounts_recursive(&spec),
    };
}
