//! Default Youki Logger

use anyhow::{bail, Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

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
pub fn init(log_format: Option<String>, log_file: Option<PathBuf>) -> Result<()> {
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
    env_logger::Builder::from_env(
        env_logger::Env::default().filter_or("YOUKI_LOG_LEVEL", DEFAULT_LOG_LEVEL),
    )
    .format(formatter)
    .target(target)
    .init();

    Ok(())
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
