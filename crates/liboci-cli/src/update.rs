use clap::Parser;

/// Update running container resource constraints
#[derive(Parser, Debug)]
pub struct Update {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,

    /// Set the maximum number of processes allowed in the container
    #[clap(long)]
    pub pids_limit: Option<i64>,
    // TODO(knight42): support more options
}
