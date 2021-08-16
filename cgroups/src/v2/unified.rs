use std::{ path::Path};

use anyhow::{Context, Result};
use oci_spec::LinuxResources;

use super::{controller_type::ControllerType};
use crate::common::{self, StringExt};

pub struct Unified{}

impl Unified {
    pub fn apply(linux_resources: &LinuxResources, cgroup_path: &Path, controllers: Vec<ControllerType>) -> Result<()> {
        if let Some(unified) = &linux_resources.unified {
            log::debug!("Apply unified cgroup config");
            for (cgroup_file, value) in unified {
                common::write_cgroup_file_str(cgroup_path.join(cgroup_file), value)
                .map_err(|e| {
                    let (subsystem, _) = cgroup_file.split_one(".").with_context(|| format!("failed to split {} with {}", cgroup_file, ".")).unwrap();
                    let context = if !controllers.iter().any(|c| c.to_string() == subsystem) {
                        format!("failed to set {} to {}: subsystem {} is not available", cgroup_file, value, subsystem)
                        } else {
                        format!("failed to set {} to {}: {}", cgroup_file, value, e)
                        };

                        e.context(context)                      
                })?; 
            }            
        }

        Ok(())
    }
}
