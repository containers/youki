//! Default Youki Logger

use anyhow::{bail, Context, Result};
use log::{LevelFilter, Log, Metadata, Record};
use once_cell::sync::OnceCell;
use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{stderr, Write};
use std::path::PathBuf;
use std::str::FromStr;

pub static LOG_FILE: OnceCell<Option<File>> = OnceCell::new();
const LOG_LEVEL_ENV_NAME: &str = "YOUKI_LOG_LEVEL";
const LOG_FORMAT_TEXT: &str = "text";
const LOG_FORMAT_JSON: &str = "json";
enum LogFormat {
    Text,
    Json,
}

/// If in debug mode, default level is debug to get maximum logging
#[cfg(debug_assertions)]
const DEFAULT_LOG_LEVEL: &str = "debug";

/// If not in debug mode, default level is warn to get important logs
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_LEVEL: &str = "warn";

/// Initialize the logger, must be called before accessing the logger
/// Multiple parts might call this at once, but the actual initialization
/// is done only once due to use of OnceCell
pub fn init(
    log_debug_flag: bool,
    log_file: Option<PathBuf>,
    log_format: Option<String>,
) -> Result<()> {
    let level = detect_log_level(log_debug_flag)
        .context("failed to parse log level")?
        .to_level();
    let format = detect_log_format(log_format).context("failed to detect log format")?;
    let _ = LOG_FILE.get_or_init(|| -> Option<File> {
        log_file.map(|path| {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path)
                .expect("failed opening log file")
        })
    });

    let logger = YoukiLogger::new(level, format);
    log::set_boxed_logger(Box::new(logger))?;

    Ok(())
}

fn detect_log_format(log_format: Option<String>) -> Result<LogFormat> {
    match log_format.as_deref() {
        None | Some(LOG_FORMAT_TEXT) => Ok(LogFormat::Text),
        Some(LOG_FORMAT_JSON) => Ok(LogFormat::Json),
        Some(unknown) => bail!("unknown log format: {}", unknown),
    }
}

fn detect_log_level(is_debug: bool) -> Result<LevelFilter> {
    let filter: Cow<str> = if is_debug {
        "debug".into()
    } else if let Ok(level) = std::env::var(LOG_LEVEL_ENV_NAME) {
        level.into()
    } else {
        DEFAULT_LOG_LEVEL.into()
    };
    Ok(LevelFilter::from_str(filter.as_ref())?)
}

struct YoukiLogger {
    /// Indicates level up to which logs are to be printed
    level: Option<log::Level>,
    format: LogFormat,
}

impl YoukiLogger {
    /// Create new logger
    pub fn new(level: Option<log::Level>, format: LogFormat) -> Self {
        Self { level, format }
    }
}

/// Implements Log interface given by log crate, so we can use its functionality
impl Log for YoukiLogger {
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
            let log_msg = match self.format {
                LogFormat::Text => text_format(record),
                LogFormat::Json => json_format(record),
            };
            // if log file is set, write to it, else write to stderr
            if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
                let _ = writeln!(log_file, "{}", log_msg);
            } else {
                let _ = writeln!(stderr(), "{}", log_msg);
            }
        }
    }

    /// Flush logs to file
    fn flush(&self) {
        if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
            log_file.flush().expect("failed to flush");
        } else {
            stderr().flush().expect("failed to flush");
        }
    }
}

fn json_format(record: &log::Record) -> String {
    serde_json::to_string(&serde_json::json!({
        "level": record.level().to_string(),
        "time": chrono::Local::now().to_rfc3339(),
        "message": record.args(),
    }))
    .expect("serde::to_string with string keys will not fail")
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

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;
    use std::env;
    struct LogLevelGuard {
        original_level: Option<String>,
    }

    impl LogLevelGuard {
        fn new(level: &str) -> Result<Self> {
            let original_level = env::var(LOG_LEVEL_ENV_NAME).ok();
            env::set_var(LOG_LEVEL_ENV_NAME, level);
            Ok(Self { original_level })
        }
    }
    impl Drop for LogLevelGuard {
        fn drop(self: &mut LogLevelGuard) {
            if let Some(level) = self.original_level.as_ref() {
                env::set_var(LOG_LEVEL_ENV_NAME, level);
            } else {
                env::remove_var(LOG_LEVEL_ENV_NAME);
            }
        }
    }

    #[test]
    fn test_detect_log_level_is_debug() {
        let _guard = LogLevelGuard::new("error").unwrap();
        assert_eq!(detect_log_level(true).unwrap(), LevelFilter::Debug)
    }

    #[test]
    #[serial]
    fn test_detect_log_level_default() {
        let _guard = LogLevelGuard::new("error").unwrap();
        env::remove_var(LOG_LEVEL_ENV_NAME);
        if cfg!(debug_assertions) {
            assert_eq!(detect_log_level(false).unwrap(), LevelFilter::Debug)
        } else {
            assert_eq!(detect_log_level(false).unwrap(), LevelFilter::Warn)
        }
    }

    #[test]
    #[serial]
    fn test_detect_log_level_from_env() {
        let _guard = LogLevelGuard::new("error").unwrap();
        assert_eq!(detect_log_level(false).unwrap(), LevelFilter::Error)
    }
}
