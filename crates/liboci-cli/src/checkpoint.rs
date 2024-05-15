use std::path::PathBuf;

use clap::Parser;

/// Checkpoint a running container
/// Reference: https://github.com/opencontainers/runc/blob/main/man/runc-checkpoint.8.md
#[derive(Parser, Debug)]
pub struct Checkpoint {
    /// Path for saving criu image files
    #[clap(long, default_value = "checkpoint")]
    pub image_path: PathBuf,
    /// Path for saving work files and logs
    #[clap(long)]
    pub work_path: Option<PathBuf>,
    /// Path for previous criu image file in pre-dump
    #[clap(long)]
    pub parent_path: Option<PathBuf>,
    /// Leave the process running after checkpointing
    #[clap(long)]
    pub leave_running: bool,
    /// Allow open tcp connections
    #[clap(long)]
    pub tcp_established: bool,
    /// Allow external unix sockets
    #[clap(long)]
    pub ext_unix_sk: bool,
    /// Allow shell jobs
    #[clap(long)]
    pub shell_job: bool,
    /// Use lazy migration mechanism
    #[clap(long)]
    pub lazy_pages: bool,
    /// Pass a file descriptor fd to criu
    #[clap(long)]
    pub status_fd: Option<u32>, // TODO: Is u32 the right type?
    /// Start a page server at the given URL
    #[clap(long)]
    pub page_server: Option<String>,
    /// Allow file locks
    #[clap(long)]
    pub file_locks: bool,
    /// Do a pre-dump
    #[clap(long)]
    pub pre_dump: bool,
    /// Cgroups mode
    #[clap(long)]
    pub manage_cgroups_mode: Option<String>,
    /// Checkpoint a namespace, but don't save its properties
    #[clap(long)]
    pub empty_ns: bool,
    /// Enable auto-deduplication
    #[clap(long)]
    pub auto_dedup: bool,

    #[clap(value_parser = clap::builder::NonEmptyStringValueParser::new(), required = true)]
    pub container_id: String,
}
