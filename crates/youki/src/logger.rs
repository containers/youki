//! Default Youki Logger

use anyhow::{bail, Context, Result};
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::metadata::LevelFilter;

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

pub fn init(
    log_debug_flag: bool,
    log_file: Option<PathBuf>,
    log_format: Option<String>,
) -> Result<()> {
    let level = detect_log_level(log_debug_flag).context("failed to parse log level")?;
    let log_format = detect_log_format(log_format).context("failed to detect log format")?;

    // I really dislike how we have to specify individual branch for each
    // combination, but I can't find any better way to do this. The tracing
    // crate makes it hard to build a single layer with different conditions.
    match (log_file, log_format) {
        (None, LogFormat::Text) => {
            // Text to stdout
            tracing_subscriber::fmt().with_max_level(level).init();
        }
        (None, LogFormat::Json) => {
            // JSON to stdout
            tracing_subscriber::fmt()
                .json()
                .flatten_event(true)
                .with_span_list(false)
                .with_max_level(level)
                .init();
        }
        (Some(path), LogFormat::Text) => {
            // Log file with text format
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path)
                .with_context(|| "failed to open log file")?;
            tracing_subscriber::fmt()
                .with_writer(file)
                .with_max_level(level)
                .init();
        }
        (Some(path), LogFormat::Json) => {
            // Log file with JSON format
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path)
                .with_context(|| "failed to open log file")?;
            tracing_subscriber::fmt()
                .json()
                .flatten_event(true)
                .with_span_list(false)
                .with_writer(file)
                .with_max_level(level)
                .init();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::{env, path::Path};

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
        assert_eq!(detect_log_level(true).unwrap(), LevelFilter::DEBUG)
    }

    #[test]
    #[serial]
    fn test_detect_log_level_default() {
        let _guard = LogLevelGuard::new("error").unwrap();
        env::remove_var(LOG_LEVEL_ENV_NAME);
        if cfg!(debug_assertions) {
            assert_eq!(detect_log_level(false).unwrap(), LevelFilter::DEBUG)
        } else {
            assert_eq!(detect_log_level(false).unwrap(), LevelFilter::WARN)
        }
    }

    #[test]
    #[serial]
    fn test_detect_log_level_from_env() {
        let _guard = LogLevelGuard::new("error").unwrap();
        assert_eq!(detect_log_level(false).unwrap(), LevelFilter::ERROR)
    }

    #[test]
    fn test_json_logfile() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let log_file = Path::join(temp_dir.path(), "test.log");
        let _guard = LogLevelGuard::new("error").unwrap();
        // Note, we can only init the tracing once, so we have to test in a
        // single unit test. The orders are important here.
        init(
            false,
            Some(log_file.to_owned()),
            Some(LOG_FORMAT_JSON.to_owned()),
        )
        .context("failed to initialize logger")?;
        assert!(
            log_file
                .as_path()
                .metadata()
                .expect("failed to get logfile metadata")
                .len()
                == 0,
            "a new logfile should be empty"
        );
        // Test that info level is not logged into the logfile because we set the log level to error.
        tracing::info!("testing this");
        if log_file
            .as_path()
            .metadata()
            .expect("failed to get logfile metadata")
            .len()
            != 0
        {
            let data = std::fs::read_to_string(&log_file).context("failed to read logfile")?;
            bail!("info level should not be logged into the logfile, but got: {data}")
        }
        // Test that the message logged is actually JSON format.
        tracing::error!("testing json log");
        let data = std::fs::read_to_string(&log_file).context("failed to read logfile")?;
        if data.is_empty() {
            bail!("logfile should not be empty")
        }
        serde_json::from_str::<serde_json::Value>(&data)
            .context(format!("failed to parse log file content: {data}"))?;
        Ok(())
    }
}
