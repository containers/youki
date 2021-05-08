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
use serde_json::json;

pub static YOUKI_LOGGER: OnceCell<YoukiLogger> = OnceCell::new();
pub static LOG_FILE: OnceCell<Option<File>> = OnceCell::new();

pub fn init(container_id: &str, log_file: Option<PathBuf>) -> Result<()> {
    let _log_file = LOG_FILE.get_or_init(|| -> Option<File> {
        if let Ok(docker_root) = env::var("YOUKI_MODE") {
            if let Some(log_file_path) = &log_file {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(log_file_path)
                    .expect("fail opening log file ");
            };

            let mut log_file_path = PathBuf::from(&docker_root);
            log_file_path.push(container_id);
            log_file_path.push(format!("{}-json.log", container_id));

            let level_filter = if let Ok(log_level_str) = env::var("YOUKI_LOG_LEVEL") {
                LevelFilter::from_str(&log_level_str).unwrap_or(LevelFilter::Warn)
            } else {
                LevelFilter::Warn
            };
            let logger = YOUKI_LOGGER.get_or_init(|| YoukiLogger::new(level_filter.to_level()));
            log::set_logger(logger)
                .map(|()| log::set_max_level(level_filter))
                .unwrap();
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(log_file_path)
                .map_err(|e| eprintln!("{:?}", e))
                .ok()
        } else {
            log_file.map(|log_file_path| {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(log_file_path)
                    .expect("fail opening log file ")
            })
        }
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
                (Some(file), Some(line)) => json!({
                    "log": format!("[{} {}:{}] {}\r\n", record.level(), file, line, record.args()),
                    "stream": "stdout",
                    "time": chrono::Local::now().to_rfc3339()
                }),
                (_, _) => json!({
                    "log": format!("[{}] {}\r\n", record.level(), record.args()),
                    "stream": "stdout",
                    "time": chrono::Local::now().to_rfc3339()
                }),
            };
            if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
                let _ = writeln!(log_file, "{}", log_msg.to_string());
            } else {
                let _ = writeln!(stderr(), "{}", log_msg.to_string());
            }
        }
    }

    fn flush(&self) {
        if let Some(mut log_file) = LOG_FILE.get().unwrap().as_ref() {
            log_file.flush().expect("Failed to flush");
        } else {
            stderr().flush().expect("Faild to flush");
        }
    }
}
