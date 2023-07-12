use clap::Parser;

/// Return the features list for a container
/// This is not a documented subcommand of runc yet, but it was introduced by
/// https://github.com/opencontainers/runc/pull/3296
#[derive(Parser, Debug)]
pub struct Features {}
