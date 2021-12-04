use clap::{App, Parser};
use clap_generate::{generate, Generator, Shell};
use std::io;

#[derive(Debug, Parser)]
pub struct CompletionParser {
    #[clap(long = "generator", short = 'g', arg_enum)]
    pub generator: Shell,
}

pub fn print_completions<G: Generator>(gen: G, app: &mut App) -> Result<(), anyhow::Error> {
    generate(gen, app, app.get_name().to_string(), &mut io::stdout());

    Ok(())
}
