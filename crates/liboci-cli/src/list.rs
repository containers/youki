use clap::Parser;

/// List created containers
#[derive(Parser, Debug)]
pub struct List {
    /// Specify the format (default or table)
    #[clap(long, default_value = "table")]
    pub format: String,

    /// Only display container IDs
    #[clap(long, short)]
    pub quiet: bool,
}
