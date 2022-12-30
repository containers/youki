use clap::Parser;
use std::path::PathBuf;

/// Update running container resource constraints
#[derive(Parser, Debug)]
pub struct Update {
    #[clap(value_parser = clap::builder::NonEmptyStringValueParser::new(), required = true)]
    pub container_id: String,

    /// Read the new resource limits from the given json file. Use - to read from stdin.
    /// If this option is used, all other options are ignored.
    #[clap(short, long)]
    pub resources: Option<PathBuf>,

    /// Set the maximum number of processes allowed in the container
    #[clap(long)]
    pub pids_limit: Option<i64>,
}
