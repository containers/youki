use std::path::PathBuf;

use crate::commands::create_cgroup_manager;
use anyhow::Result;
use libcgroups::{self, common::ControllerOpt};
use liboci_cli::Update;
use oci_spec::runtime::{LinuxPids, LinuxResources};

pub fn update(args: Update, root_path: PathBuf) -> Result<()> {
    let cmanager = create_cgroup_manager(root_path, &args.container_id)?;

    let mut linux_res = LinuxResources::default();
    if let Some(new_pids_limit) = args.pids_limit {
        let mut pids = LinuxPids::default();
        pids.set_limit(new_pids_limit);
        linux_res.set_pids(Some(pids));
    }

    cmanager.apply(&ControllerOpt {
        resources: &linux_res,
        disable_oom_killer: false,
        oom_score_adj: None,
        freezer_state: None,
    })?;
    Ok(())
}
