use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::anyhow;
use regex::Regex;

use crate::{
    cgroups::Controller,
};
use oci_spec::{LinuxHugepageLimit, LinuxResources};

pub struct Hugetlb {}

impl Controller for Hugetlb {
    fn apply(
        linux_resources: &LinuxResources,
        cgroup_root: &std::path::Path,
        pid: nix::unistd::Pid,
    ) -> anyhow::Result<()> {
        log::debug!("Apply Hugetlb cgroup config");
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
                let page_size: u64 = caps["pagesize"].parse()?;
                if !Self::is_power_of_two(page_size) {
                    return Err(anyhow!("page size must be in the format of 2^(integer)"));
                }
            }
        }

        Self::write_file(
            &root_path.join(format!("hugetlb.{}.limit_in_bytes", hugetlb.page_size)),
            &hugetlb.limit.to_string(),
        )?;
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
        (number != 0) && (number & (number - 1)) == 0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use oci_spec::LinuxHugepageLimit;

    fn set_fixture(temp_dir: &std::path::Path, filename: &str, val: &str) -> anyhow::Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(temp_dir.join(filename))?
            .write_all(val.as_bytes())?;

        Ok(())
    }

    fn create_temp_dir(test_name: &str) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_set_hugetlb() {
        let page_file_name = "hugetlb.2MB.limit_in_bytes";
        let tmp = create_temp_dir("test_set_hugetlb").expect("create temp directory for test");
        set_fixture(&tmp, page_file_name, "0").expect("Set fixture for 2 MB page size");

        let hugetlb = LinuxHugepageLimit {
            page_size: "2MB".to_owned(),
            limit: 16384,
        };
        Hugetlb::apply(&tmp, &hugetlb).expect("apply hugetlb");
        let content =
            std::fs::read_to_string(tmp.join(page_file_name)).expect("Read hugetlb file content");
        assert_eq!(hugetlb.limit.to_string(), content);
    }

    #[test]
    fn test_set_hugetlb_with_invalid_page_size() {
        let tmp = create_temp_dir("test_set_hugetlb_with_invalid_page_size")
            .expect("create temp directory for test");

        let hugetlb = LinuxHugepageLimit {
            page_size: "3MB".to_owned(),
            limit: 16384,
        };

        let result = Hugetlb::apply(&tmp, &hugetlb);
        assert!(
            result.is_err(),
            "page size that is not a power of two should be an error"
        );
    }
}
