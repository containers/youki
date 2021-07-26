use std::{collections::HashMap, fs, path::Path};

use anyhow::{bail, Result};

use crate::cgroups::{
    common,
    stats::{HugeTlbStats, StatsProvider},
    v1::Controller,
};
use oci_spec::{LinuxHugepageLimit, LinuxResources};

pub struct Hugetlb {}

impl Controller for Hugetlb {
    type Resource = Vec<LinuxHugepageLimit>;

    fn apply(linux_resources: &LinuxResources, cgroup_root: &std::path::Path) -> Result<()> {
        log::debug!("Apply Hugetlb cgroup config");

        if let Some(hugepage_limits) = Self::needs_to_handle(linux_resources) {
            for hugetlb in hugepage_limits {
                Self::apply(cgroup_root, hugetlb)?
            }
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if !linux_resources.hugepage_limits.is_empty() {
            return Some(&linux_resources.hugepage_limits);
        }

        None
    }
}

impl StatsProvider for Hugetlb {
    type Stats = HashMap<String, HugeTlbStats>;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let page_sizes = Self::supported_page_sizes()?;
        let mut hugetlb_stats = HashMap::with_capacity(page_sizes.len());

        for page_size in &page_sizes {
            let stats = Self::stats_for_page_size(cgroup_path, page_size)?;
            hugetlb_stats.insert(page_size.to_owned(), stats);
        }

        Ok(hugetlb_stats)
    }
}

impl Hugetlb {
    fn apply(root_path: &Path, hugetlb: &LinuxHugepageLimit) -> Result<()> {
        let page_size: String = hugetlb
            .page_size
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
        let page_size: u64 = page_size.parse()?;
        if !Self::is_power_of_two(page_size) {
            bail!("page size must be in the format of 2^(integer)");
        }

        common::write_cgroup_file(
            root_path.join(format!("hugetlb.{}.limit_in_bytes", hugetlb.page_size)),
            hugetlb.limit,
        )?;
        Ok(())
    }

    fn is_power_of_two(number: u64) -> bool {
        (number != 0) && (number & (number - 1)) == 0
    }

    fn supported_page_sizes() -> Result<Vec<String>> {
        let mut sizes = Vec::new();
        for hugetlb_entry in fs::read_dir("/sys/kernel/mm/hugepages")? {
            let hugetlb_entry = hugetlb_entry?;
            if !hugetlb_entry.path().is_dir() {
                continue;
            }

            let file_name = hugetlb_entry.file_name();
            let file_name = file_name.to_str().unwrap();
            if let Some(name_stripped) = file_name.strip_prefix("hugepages-") {
                if let Some(size) = name_stripped.strip_suffix("kB") {
                    let size: u64 = size.parse()?;

                    let size_moniker = if size >= (1 << 20) {
                        (size >> 20).to_string() + "GB"
                    } else if size >= (1 << 10) {
                        (size >> 10).to_string() + "MB"
                    } else {
                        size.to_string() + "KB"
                    };

                    sizes.push(size_moniker);
                }
            }
        }

        Ok(sizes)
    }

    fn stats_for_page_size(cgroup_path: &Path, page_size: &str) -> Result<HugeTlbStats> {
        let mut stats = HugeTlbStats::default();

        let usage_file = format!("hugetlb.{}.usage_in_bytes", page_size);
        let usage_content = common::read_cgroup_file(cgroup_path.join(usage_file))?;
        stats.usage = usage_content.trim().parse()?;

        let max_file = format!("hugetlb.{}.max_usage_in_bytes", page_size);
        let max_content = common::read_cgroup_file(cgroup_path.join(max_file))?;
        stats.max_usage = max_content.trim().parse()?;

        let failcnt_file = format!("hugetlb.{}.failcnt", page_size);
        let failcnt_content = common::read_cgroup_file(cgroup_path.join(failcnt_file))?;
        stats.fail_count = failcnt_content.trim().parse()?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
    use oci_spec::LinuxHugepageLimit;
    use std::fs::read_to_string;

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
        let content = read_to_string(tmp.join(page_file_name)).expect("Read hugetlb file content");
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

    quickcheck! {
        fn property_test_set_hugetlb(hugetlb: LinuxHugepageLimit) -> bool {
            let page_file_name = format!("hugetlb.{:?}.limit_in_bytes", hugetlb.page_size);
            let tmp = create_temp_dir("property_test_set_hugetlb").expect("create temp directory for test");
            set_fixture(&tmp, &page_file_name, "0").expect("Set fixture for page size");

            let result = Hugetlb::apply(&tmp, &hugetlb);

            let page_size: String = hugetlb
            .page_size
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
            let page_size: u64 = page_size.parse().expect("parse page size");

            if Hugetlb::is_power_of_two(page_size) && page_size != 1 {
                let content =
                    read_to_string(tmp.join(page_file_name)).expect("Read hugetlb file content");
                hugetlb.limit.to_string() == content
            } else {
                result.is_err()
            }
        }
    }
}
