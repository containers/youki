use std::{fs::{self, OpenOptions}, io::Write, path::Path};

use crate::{cgroups::Controller, spec::{ LinuxBlockIo, LinuxResources}};

const CGROUP_BLKIO_THROTTLE_READ_DPS: &str = "blkio.throttle.read_bps_device";
const CGROUP_BLKIO_THROTTLE_WRITE_DPS: &str = "blkio.throttle.write_bps_device";
const CGROUP_BLKIO_THROTTLE_READ_IOPS: &str = "blkio.throttle.read_iops_device";
const CGROUP_BLKIO_THROTTLE_WRITE_IOPS: &str = "blkio.throttle.write_iops_device";

pub struct Blkio {}

impl Controller for Blkio {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: nix::unistd::Pid) -> anyhow::Result<()> {      
       match &linux_resources.block_io {
           None => return Ok(()),
           Some(block_io) => {
                fs::create_dir_all(cgroup_root)?;
                Self::apply(cgroup_root, block_io)?;
           }
       }

       OpenOptions::new()
       .create(false)
       .write(true)
       .truncate(false)
       .open(cgroup_root.join("cgroup.procs"))?
       .write_all(pid.to_string().as_bytes())?;

       Ok(())
    }
}

impl Blkio {
    fn apply(root_path: &Path, blkio: &LinuxBlockIo) -> anyhow::Result<()> {
        for trbd in &blkio.blkio_throttle_read_bps_device {
            Self::write_file(&root_path.join(CGROUP_BLKIO_THROTTLE_READ_DPS), &format!("{}:{} {}", trbd.major, trbd.minor, trbd.rate))?;
        }

        for twbd in &blkio.blkio_throttle_write_bps_device {
            Self::write_file(&root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_DPS), &format!("{}:{} {}", twbd.major, twbd.minor, twbd.rate))?;
        }

        for trid in &blkio.blkio_throttle_read_iops_device {
            Self::write_file(&root_path.join(CGROUP_BLKIO_THROTTLE_READ_IOPS), &format!("{}:{} {}", trid.major, trid.minor, trid.rate))?;
        }

        for twid in &blkio.blkio_throttle_write_iops_device {
            Self::write_file(&root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_IOPS), &format!("{}:{} {}", twid.major, twid.minor, twid.rate))?;
        }

        Ok(())
    }

    fn write_file(file_path: &Path, data: &str) -> anyhow::Result<()> {       
        fs::OpenOptions::new()
            .create(false)
            .append(true)
            .truncate(false)
            .open(file_path)?
            .write_all(data.as_bytes())?;

        Ok(())
    }
}