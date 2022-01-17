use clap::Parser;
use std::path::PathBuf;

/// Checkpoints a running container
#[derive(Parser, Debug)]
pub struct Checkpoint {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,

    #[clap(long)]
    pub image_path: PathBuf,

    #[clap(long)]
    pub work_path: PathBuf,

    #[clap(long)]
    pub leave_running: bool,

    #[clap(long)]
    pub shell_job: bool,
}
