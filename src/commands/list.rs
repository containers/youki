//! Contains Functionality of list container command
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Local};
use clap::Clap;
use tabwriter::TabWriter;

use crate::container::{state::State, Container};

/// List created containers
#[derive(Clap, Debug)]
pub struct List {}

impl List {
    /// lists all existing containers
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let root_path = fs::canonicalize(root_path)?;
        let mut content = String::new();
        // all containers' data is stored in their respective dir in root directory
        // so we iterate through each and print the various info
        for container_dir in fs::read_dir(root_path)? {
            let container_dir = container_dir?.path();
            let state_file = State::file_path(&container_dir);
            if !state_file.exists() {
                continue;
            }

            let container = Container::load(container_dir)?;
            let pid = if let Some(pid) = container.pid() {
                pid.to_string()
            } else {
                "".to_owned()
            };

            let user_name = container.creator().unwrap_or_default();

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
                container.bundle().display(),
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
