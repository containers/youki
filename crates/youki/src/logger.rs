//! Default Youki Logger

use std::env;
use std::io::{stderr, Write};
use std::path::PathBuf;
use std::{
    fs::{File, OpenOptions},
    str::FromStr,
};

use anyhow::Result;
use log::{LevelFilter, Log, Metadata, Record};
use once_cell::sync::OnceCell;

/// Public global variables to access logger and logfile
pub static YOUKI_LOGGER: OnceCell<YoukiLogger> = OnceCell::new();
pub static LOG_FILE: OnceCell<Option<File>> = OnceCell::new();

/// If in debug mode, default level is debug to get maximum logging
#[cfg(debug_assertions)]
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Debug;

/// If not in debug mode, default level is warn to get important logs
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Warn;

/// Initialize the logger, must be called before accessing the logger
/// Multiple parts might call this at once, but the actual initialization
/// is done only once due to use of OnceCell
pub fn init(log_file: Option<PathBuf>) -> Result<()> {
    // If file exists, ignore, else create and open the file
    let _log_file = LOG_FILE.get_or_init(|| -> Option<File> {
        // set the log level if specified in env variable or set to default
        let level_filter = if let Ok(log_level_str) = env::var("YOUKI_LOG_LEVEL") {
            LevelFilter::from_str(&log_level_str).unwrap_or(DEFAULT_LOG_LEVEL)
        } else {
            DEFAULT_LOG_LEVEL
        };

        // Create a new logger, or get existing if already created
        let logger = YOUKI_LOGGER.get_or_init(|| YoukiLogger::new(level_filter.to_level()));

        log::set_logger(logger)
            .map(|()| log::set_max_level(level_filter))
            .expect("set logger failed");

        // Create and open log file
        log_file.as_ref().map(|log_file_path| {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(log_file_path)
                .expect("failed opening log file ")
        })
    });
    Ok(())
}

/// Youki's custom Logger
pub struct YoukiLogger {
    /// Indicates level up to which logs are to be printed
    level: Option<log::Level>,
}

impl YoukiLogger {
    /// Create new logger
    pub fn new(level: Option<log::Level>) -> Self {
        Self { level }
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
            log_file.flush().expect("Failed to flush");
        } else {
            stderr().flush().expect("Failed to flush");
        }
    }
}
