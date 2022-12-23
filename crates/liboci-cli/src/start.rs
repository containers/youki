use clap::Parser;

/// Start a previously created container
#[derive(Parser, Debug)]
pub struct Start {
    #[clap(value_parser = clap::builder::NonEmptyStringValueParser::new(), required = true)]
    pub container_id: String,
}
