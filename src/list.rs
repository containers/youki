use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Local};
use clap::Clap;
use tabwriter::TabWriter;

use crate::container::Container;

#[derive(Clap, Debug)]
pub struct List {}

impl List {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let root_path = fs::canonicalize(root_path)?;
        let mut content = String::new();

        for container_dir in fs::read_dir(root_path)? {
            let container_dir = container_dir?.path();
            let state_file = container_dir.join("state.json");
            if !state_file.exists() {
                continue;
            }

            let container = Container::load(container_dir)?.refresh_status()?;
            let pid = if let Some(pid) = container.pid() {
                pid.to_string()
            } else {
                "".to_owned()
            };

            let user_name = if let Some(creator) = container.creator() {
                creator
            } else {
                OsString::new()
            };

            let created = if let Some(utc) = container.created() {
                let local: DateTime<Local> = DateTime::from(utc);
                local.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
            } else {
                "".to_owned()
            };

            content.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                container.id(),
                pid,
                container.status(),
                container.bundle(),
                created,
                user_name.to_string_lossy()
            ));
        }

        let mut tab_writer = TabWriter::new(io::stdout());
        writeln!(&mut tab_writer, "ID\tPID\tSTATUS\tBUNDLE\tCREATED\tCREATOR")?;
        write!(&mut tab_writer, "{}", content)?;
        tab_writer.flush()?;

        Ok(())
    }
}
