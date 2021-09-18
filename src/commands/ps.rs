use crate::{container::Container, utils};
use anyhow::{bail, Context, Result};
use cgroups;
use clap::{self, Clap};
use std::{path::PathBuf, process::Command};

/// display the processes inside a container
#[derive(Clap, Debug)]
pub struct Ps {
    /// format to display processes: table or json (default: "table")
    #[clap(short, long, default_value = "table")]
    format: String,
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
    /// options will be passed to the ps utility
    #[clap(setting = clap::ArgSettings::Last)]
    ps_options: Vec<String>,
}
impl Ps {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_root = root_path.join(&self.container_id);
        if !container_root.exists() {
            bail!("{} doesn't exist.", self.container_id)
        }
        let container = Container::load(container_root)?.refresh_status()?;
        if container.root.exists() {
            let config_absolute_path = container.root.join("config.json");
            log::debug!("load spec from {:?}", config_absolute_path);
            let spec = oci_spec::runtime::Spec::load(config_absolute_path)?;
            log::debug!("spec: {:?}", spec);
            let cgroups_path = utils::get_cgroup_path(
                &spec.linux.context("no linux in spec")?.cgroups_path,
                container.id(),
            );
            let systemd_cgroup = container
                .systemd()
                .context("could not determine cgroup manager")?;
            let cmanager = cgroups::common::create_cgroup_manager(cgroups_path, systemd_cgroup)?;
            let pids: Vec<i32> = cmanager
                .get_all_pids()?
                .iter()
                .map(|pid| pid.as_raw())
                .collect();

            if self.format == "json" {
                println!("{}", serde_json::to_string(&pids)?);
            } else if self.format == "table" {
                let default_ps_options = vec![String::from("-ef")];
                let ps_options = if self.ps_options.is_empty() {
                    &default_ps_options
                } else {
                    &self.ps_options
                };
                let output = Command::new("ps").args(ps_options).output()?;
                if !output.status.success() {
                    println!("{}", std::str::from_utf8(&output.stderr)?);
                } else {
                    let lines = std::str::from_utf8(&output.stdout)?;
                    let lines: Vec<&str> = lines.split('\n').collect();
                    let pid_index = get_pid_index(lines[0])?;
                    println!("{}", &lines[0]);
                    for line in &lines[1..] {
                        if line.is_empty() {
                            continue;
                        }
                        let fields: Vec<&str> = line.split_whitespace().collect();
                        let pid: i32 = fields[pid_index].parse()?;
                        if pids.contains(&pid) {
                            println!("{}", line);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn get_pid_index(title: &str) -> Result<usize> {
    let titles = title.split_whitespace();

    for (index, name) in titles.enumerate() {
        if name == "PID" {
            return Ok(index);
        }
    }
    bail!("could't find PID field in ps output");
}
