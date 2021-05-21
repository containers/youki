use std::{fs::{self, OpenOptions}, io::Write, path::Path};

use regex::Regex;
use anyhow::anyhow;

use crate::{cgroups::Controller, spec::{ LinuxHugepageLimit, LinuxResources}};

pub struct Hugetlb {}

impl Controller for Hugetlb {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &std::path::Path, pid: nix::unistd::Pid) -> anyhow::Result<()> {
        fs::create_dir_all(cgroup_root)?;

        for hugetlb in &linux_resources.hugepage_limits {
            Self::apply(cgroup_root, hugetlb)?
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

impl Hugetlb {
    fn apply(root_path: &Path, hugetlb: &LinuxHugepageLimit) -> anyhow::Result<()> {
        let re = Regex::new(r"(?P<pagesize>[0-9]+)[KMG]B")?;
        let caps = re.captures(&hugetlb.page_size);
        match caps {
            None => return Err(anyhow!("page size must be in the format [0-9]+[KMG]B")),
            Some(caps) => {
                let page_size:u64 = caps["pagesize"].parse()?;
                if !Self::is_power_of_two(page_size) {
                    return Err(anyhow!("page size must be in the format of 2^(integer)"))
                }
            }
        }

        Self::write_file(&root_path.join(format!("hugetlb.{}.limit_in_bytes", hugetlb.page_size)), &hugetlb.limit.to_string())?;
        Ok(())
    }

    fn write_file(file_path: &Path, data: &str) -> anyhow::Result<()> {       
        fs::OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path)?
            .write_all(data.as_bytes())?;

        Ok(())
    }

    fn is_power_of_two(number: u64) -> bool {
        (number != 0) && (number & (number -1)) == 0
    }
}