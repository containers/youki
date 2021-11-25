//! Default Youki Logger

use anyhow::{bail, Context, Result};
use log::LevelFilter;
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

const LOG_LEVEL_ENV_NAME: &str = "YOUKI_LOG_LEVEL";

/// If in debug mode, default level is debug to get maximum logging
#[cfg(debug_assertions)]
const DEFAULT_LOG_LEVEL: &str = "debug";

/// If not in debug mode, default level is warn to get important logs
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_LEVEL: &str = "warn";

const LOG_FORMAT_TEXT: &str = "text";
const LOG_FORMAT_JSON: &str = "json";

/// Initialize the logger, must be called before accessing the logger
/// Multiple parts might call this at once, but the actual initialization
/// is done only once due to use of OnceCell
pub fn init(
    log_debug_flag: bool,
    log_file: Option<PathBuf>,
    log_format: Option<String>,
) -> Result<()> {
    let log_level = detect_log_level(log_debug_flag);
    let formatter = match log_format.as_deref() {
        None | Some(LOG_FORMAT_TEXT) => text_write,
        Some(LOG_FORMAT_JSON) => json_write,
        Some(unknown) => bail!("unknown log format: {}", unknown),
    };
    let target = if let Some(log_file) = log_file {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(log_file)
            .context("failed opening log file")?;
        env_logger::Target::Pipe(Box::new(file))
    } else {
        env_logger::Target::Stderr
    };
    env_logger::Builder::new()
        .filter_level(log_level.context("failed to parse log level")?)
        .format(formatter)
        .target(target)
        .init();

    Ok(())
}

fn detect_log_level(is_debug: bool) -> Result<LevelFilter> {
    let filter: Cow<str> = if is_debug {
        dbg!(is_debug);
        "debug".into()
    } else if let Ok(level) = std::env::var(LOG_LEVEL_ENV_NAME) {
        println!("from env: {:?}", level);
        level.into()
    } else {
        println!("default: {:?}", DEFAULT_LOG_LEVEL);
        DEFAULT_LOG_LEVEL.into()
    };
    Ok(LevelFilter::from_str(filter.as_ref())?)
}

fn json_write<F: 'static>(f: &mut F, record: &log::Record) -> std::io::Result<()>
where
    F: Write,
{
    write!(f, "{{")?;
    write!(f, "\"level\":\"{}\",", record.level())?;
    write!(f, "\"time\":\"{}\"", chrono::Local::now().to_rfc3339(),)?;
    write!(f, ",\"msg\":")?;
    // Use serde_json here so we don't have to worry about escaping special characters in the string.
    serde_json::to_writer(f.by_ref(), &record.args().to_string())?;
    writeln!(f, "}}")?;

    Ok(())
}

fn text_write<F: 'static>(f: &mut F, record: &log::Record) -> std::io::Result<()>
where
    F: Write,
{
    match (record.file(), record.line()) {
        (Some(file), Some(line)) => {
            write!(f, "[{} {}:{}]", record.level(), file, line)?;
        }
        (_, _) => write!(f, "[{}]", record.level(),)?,
    };
    write!(
        f,
        " {} {}\r\n",
        chrono::Local::now().to_rfc3339(),
        record.args()
    )?;

    Ok(())
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
