use anyhow::{Context, Result};
use std::borrow::Cow;
use std::str::FromStr;
use tracing::metadata::LevelFilter;

const LOG_LEVEL_ENV_NAME: &str = "YOUKI_INTEGRATION_LOG_LEVEL";

/// Initialize the logger, must be called before accessing the logger
/// Multiple parts might call this at once, but the actual initialization
/// is done only once due to use of OnceCell
pub fn init(debug: bool) -> Result<()> {
    let level = detect_log_level(debug).context("failed to parse log level")?;
    tracing_subscriber::fmt().with_max_level(level).init();

    Ok(())
}

fn detect_log_level(is_debug: bool) -> Result<LevelFilter> {
    let filter: Cow<str> = if is_debug {
        "debug".into()
    } else if let Ok(level) = std::env::var(LOG_LEVEL_ENV_NAME) {
        level.into()
    } else {
        "off".into()
    };

    Ok(LevelFilter::from_str(filter.as_ref())?)
}
