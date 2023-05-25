//! Default Youki Logger

use anyhow::{bail, Context, Result};
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::Level;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

fn detect_log_format(log_format: Option<&str>) -> Result<LogFormat> {
    match log_format {
        None | Some(LOG_FORMAT_TEXT) => Ok(LogFormat::Text),
        Some(LOG_FORMAT_JSON) => Ok(LogFormat::Json),
        Some(unknown) => bail!("unknown log format: {}", unknown),
    }
}

fn detect_log_level(is_debug: bool) -> Result<Level> {
    let filter: Cow<str> = if is_debug {
        "debug".into()
    } else if let Ok(level) = std::env::var(LOG_LEVEL_ENV_NAME) {
        level.into()
    } else {
        DEFAULT_LOG_LEVEL.into()
    };
    Ok(Level::from_str(filter.as_ref())?)
}

pub struct ObservabilityConfig {
    pub log_debug_flag: bool,
    pub log_file: Option<PathBuf>,
    pub log_format: Option<String>,
}

impl From<&crate::Opts> for ObservabilityConfig {
    fn from(opts: &crate::Opts) -> Self {
        Self {
            log_debug_flag: opts.global.debug,
            log_file: opts.global.log.to_owned(),
            log_format: opts.global.log_format.to_owned(),
        }
    }
}

pub fn init_observability<T>(config: T) -> Result<()>
where
    T: Into<ObservabilityConfig>,
{
    let config = config.into();
    let level =
        detect_log_level(config.log_debug_flag).with_context(|| "failed to parse log level")?;
    let log_level_filter = tracing_subscriber::filter::LevelFilter::from(level);
    let log_format = detect_log_format(config.log_format.as_deref())
        .with_context(|| "failed to detect log format")?;

    let subscriber = tracing_subscriber::registry().with(log_level_filter);

    // I really dislike how we have to specify individual branch for each
    // combination, but I can't find any better way to do this. The tracing
    // crate makes it hard to build a single layer with different conditions.
    match (config.log_file.as_ref(), log_format) {
        (None, LogFormat::Text) => {
            // Text to stderr
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .without_time()
                        .with_writer(std::io::stderr),
                )
                .try_init()
                .map_err(|e| anyhow::anyhow!("failed to init logger: {}", e))?;
        }
        (None, LogFormat::Json) => {
            // JSON to stderr
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_span_list(false)
                        .with_writer(std::io::stderr),
                )
                .try_init()
                .map_err(|e| anyhow::anyhow!("failed to init logger: {}", e))?;
        }
        (Some(path), LogFormat::Text) => {
            // Log file with text format
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path)
                .with_context(|| "failed to open log file")?;
            subscriber
                .with(tracing_subscriber::fmt::layer().with_writer(file))
                .try_init()
                .map_err(|e| anyhow::anyhow!("failed to init logger: {}", e))?;
        }
        (Some(path), LogFormat::Json) => {
            // Log file with JSON format
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path)
                .with_context(|| "failed to open log file")?;
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_span_list(false)
                        .with_writer(file),
                )
                .try_init()
                .map_err(|e| anyhow::anyhow!("failed to init logger: {}", e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use libcontainer::test_utils::TestCallbackError;
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
        assert_eq!(detect_log_level(true).unwrap(), tracing::Level::DEBUG)
    }

    #[test]
    #[serial]
    fn test_detect_log_level_default() {
        let _guard = LogLevelGuard::new("error").unwrap();
        env::remove_var(LOG_LEVEL_ENV_NAME);
        if cfg!(debug_assertions) {
            assert_eq!(detect_log_level(false).unwrap(), tracing::Level::DEBUG)
        } else {
            assert_eq!(detect_log_level(false).unwrap(), tracing::Level::WARN)
        }
    }

    #[test]
    #[serial]
    fn test_detect_log_level_from_env() {
        let _guard = LogLevelGuard::new("error").unwrap();
        assert_eq!(detect_log_level(false).unwrap(), tracing::Level::ERROR)
    }

    #[test]
    fn test_init_many_times() -> Result<()> {
        let cb = || {
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            let log_file = Path::join(temp_dir.path(), "test.log");
            let _guard = LogLevelGuard::new("error").unwrap();
            let config = ObservabilityConfig {
                log_debug_flag: false,
                log_file: Some(log_file),
                log_format: None,
            };
            init_observability(config).map_err(|err| TestCallbackError::Other(err.into()))?;
            Ok(())
        };
        libcontainer::test_utils::test_in_child_process(cb)
            .with_context(|| "failed the first init tracing")?;
        libcontainer::test_utils::test_in_child_process(cb)
            .with_context(|| "failed the second init tracing")?;
        Ok(())
    }

    #[test]
    fn test_higher_loglevel_no_log() -> Result<()> {
        libcontainer::test_utils::test_in_child_process(|| {
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            let log_file = Path::join(temp_dir.path(), "test.log");
            let _guard = LogLevelGuard::new("error").unwrap();
            // Note, we can only init the tracing once, so we have to test in a
            // single unit test. The orders are important here.
            let config = ObservabilityConfig {
                log_debug_flag: false,
                log_file: Some(log_file.clone()),
                log_format: None,
            };
            init_observability(config).map_err(|err| TestCallbackError::Other(err.into()))?;
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
                .map_err(|err| format!("failed to get logfile metadata: {err:?}"))?
                .len()
                != 0
            {
                let data = std::fs::read_to_string(&log_file)
                    .map_err(|err| format!("failed to read the logfile: {err:?}"))?;
                Err(TestCallbackError::Custom(format!(
                    "info level should not be logged into the logfile, but got: {data}"
                )))?;
            }

            Ok(())
        })?;

        Ok(())
    }

    #[test]
    fn test_json_logfile() -> Result<()> {
        libcontainer::test_utils::test_in_child_process(|| {
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            let log_file = Path::join(temp_dir.path(), "test.log");
            let _guard = LogLevelGuard::new("error").unwrap();
            // Note, we can only init the tracing once, so we have to test in a
            // single unit test. The orders are important here.
            let config = ObservabilityConfig {
                log_debug_flag: false,
                log_file: Some(log_file.clone()),
                log_format: Some(LOG_FORMAT_JSON.to_owned()),
            };
            init_observability(config).map_err(|err| TestCallbackError::Other(err.into()))?;
            assert!(
                log_file
                    .as_path()
                    .metadata()
                    .expect("failed to get logfile metadata")
                    .len()
                    == 0,
                "a new logfile should be empty"
            );
            // Test that the message logged is actually JSON format.
            tracing::error!("testing json log");
            let data = std::fs::read_to_string(&log_file)
                .map_err(|err| format!("failed to read the logfile: {err:?}"))?;
            if data.is_empty() {
                Err("logfile should not be empty")?;
            }
            serde_json::from_str::<serde_json::Value>(&data)
                .map_err(|err| format!("failed to parse {data}: {err:?}"))?;
            Ok(())
        })?;

        Ok(())
    }
}
