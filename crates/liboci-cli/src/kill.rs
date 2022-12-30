use clap::Parser;

/// Send the specified signal to the container
#[derive(Parser, Debug)]
pub struct Kill {
    #[clap(value_parser = clap::builder::NonEmptyStringValueParser::new(), required = true)]
    pub container_id: String,
    pub signal: String,
    #[clap(short, long)]
    pub all: bool,
}
