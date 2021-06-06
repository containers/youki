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

pub static YOUKI_LOGGER: OnceCell<YoukiLogger> = OnceCell::new();
pub static LOG_FILE: OnceCell<Option<File>> = OnceCell::new();

pub fn init(log_file: Option<PathBuf>) -> Result<()> {
    let _log_file = LOG_FILE.get_or_init(|| -> Option<File> {
        let level_filter = if let Ok(log_level_str) = env::var("YOUKI_LOG_LEVEL") {
            LevelFilter::from_str(&log_level_str).unwrap_or(LevelFilter::Warn)
        } else {
            LevelFilter::Warn
        };

        let logger = YOUKI_LOGGER.get_or_init(|| YoukiLogger::new(level_filter.to_level()));
        log::set_logger(logger)
            .map(|()| log::set_max_level(level_filter))
            .expect("set logger failed");
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
pub struct YoukiLogger {
    level: Option<log::Level>,
}

impl YoukiLogger {
    pub fn new(level: Option<log::Level>) -> Self {
        Self { level }
    }
}

impl Log for YoukiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if let Some(level) = self.level {
            metadata.level() <= level
        } else {
            false
        }
    }

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
            if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
                let _ = writeln!(log_file, "{}", log_msg);
            } else {
                let _ = writeln!(stderr(), "{}", log_msg);
            }
        }
    }

    fn flush(&self) {
        if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
            log_file.flush().expect("Failed to flush");
        } else {
            stderr().flush().expect("Failed to flush");
        }
    }
}
