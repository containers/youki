use anyhow::{Context, Result};
use log::{LevelFilter, Log, Metadata, Record};
use std::borrow::Cow;
use std::io::{stderr, Write};
use std::str::FromStr;

const LOG_LEVEL_ENV_NAME: &str = "YOUKI_INTEGRATION_LOG_LEVEL";

/// Initialize the logger, must be called before accessing the logger
/// Multiple parts might call this at once, but the actual initialization
/// is done only once due to use of OnceCell
pub fn init(debug: bool) -> Result<()> {
    let level = detect_log_level(debug).context("failed to parse log level")?;
    let logger = IntegrationLogger::new(level.to_level());
    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(level))
        .expect("set logger failed");

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

struct IntegrationLogger {
    /// Indicates level up to which logs are to be printed
    level: Option<log::Level>,
}

impl IntegrationLogger {
    /// Create new logger
    pub fn new(level: Option<log::Level>) -> Self {
        Self { level }
    }
}

/// Implements Log interface given by log crate, so we can use its functionality
impl Log for IntegrationLogger {
    /// Check if level of given log is enabled or not
    fn enabled(&self, metadata: &Metadata) -> bool {
        if let Some(level) = self.level {
            metadata.level() <= level
        } else {
            false
        }
    }

    /// Function to carry out logging
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log_msg = text_format(record);
            // if log file is set, write to it, else write to stderr
            let _ = writeln!(stderr(), "{log_msg}");
        }
    }

    /// Flush logs to file
    fn flush(&self) {
        stderr().flush().expect("failed to flush");
    }
}

fn text_format(record: &log::Record) -> String {
    let log_msg = match (record.file(), record.line()) {
        (Some(file), Some(line)) => format!(
            "[{} {}:{}] {} {}\r",
            record.level(),
            file,
            line,
            chrono::Local::now().to_rfc3339(),
            record.args()
        ),
        (_, _) => format!(
            "[{}] {} {}\r",
            record.level(),
            chrono::Local::now().to_rfc3339(),
            record.args()
        ),
    };

    log_msg
}
