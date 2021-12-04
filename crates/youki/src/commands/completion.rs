use anyhow::Result;
use clap::{App, Parser};
use clap_generate::{generate, Shell};
use std::io;

#[derive(Debug, Parser)]
/// Generate scripts for shell completion
pub struct Completion {
    #[clap(long = "shell", short = 's', arg_enum)]
    pub shell: Shell,
}

pub fn completion(args: Completion, app: &mut App) -> Result<()> {
    generate(
        args.shell,
        app,
        app.get_name().to_string(),
        &mut io::stdout(),
    );

    Ok(())
}
