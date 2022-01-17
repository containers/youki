use clap::Parser;
use std::path::PathBuf;

/// Restore a container from a checkpoint
#[derive(Parser, Debug)]
pub struct Restore {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,

    #[clap(long)]
    pub image_path: PathBuf,

    #[clap(long)]
    pub work_path: PathBuf,

    #[clap(long)]
    pub bundle: PathBuf,

    #[clap(long)]
    pub pid_file: PathBuf,

    #[clap(long)]
    pub shell_job: bool,
}
