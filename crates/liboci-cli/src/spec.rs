use clap::Parser;
use std::path::PathBuf;

/// Command generates a config.json
#[derive(Parser, Debug)]
pub struct Spec {
    /// Set path to the root of the bundle directory
    #[clap(long, short)]
    pub bundle: Option<PathBuf>,

    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}
