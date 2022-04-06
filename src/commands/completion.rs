use anyhow::Result;
use clap::{Command, Parser};

use clap_complete::{generate, Shell};
use std::io;

#[derive(Debug, Parser)]
/// Generate scripts for shell completion
pub struct Completion {
    #[clap(long = "shell", short = 's', arg_enum)]
    pub shell: Shell,
}

pub fn completion(args: Completion, app: &mut Command) -> Result<()> {
    generate(
        args.shell,
        app,
        app.get_name().to_string(),
        &mut io::stdout(),
    );

    Ok(())
}
