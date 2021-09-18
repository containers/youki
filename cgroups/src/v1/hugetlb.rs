use std::{collections::HashMap, path::Path};

use anyhow::{bail, Context, Result};

use crate::{
    common::{self, ControllerOpt},
    stats::{supported_page_sizes, HugeTlbStats, StatsProvider},
};

use super::Controller;
use oci_spec::runtime::LinuxHugepageLimit;

pub struct HugeTlb {}

impl Controller for HugeTlb {
    type Resource = Vec<LinuxHugepageLimit>;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &std::path::Path) -> Result<()> {
        log::debug!("Apply Hugetlb cgroup config");

        if let Some(hugepage_limits) = Self::needs_to_handle(controller_opt) {
            for hugetlb in hugepage_limits {
                Self::apply(cgroup_root, hugetlb)
                    .context("failed to apply hugetlb resource restrictions")?
            }
        }

        Ok(())
    }

    fn needs_to_handle(controller_opt: &ControllerOpt) -> Option<&Self::Resource> {
        if let Some(hugepage_limits) = controller_opt.resources.hugepage_limits().as_ref() {
            if !hugepage_limits.is_empty() {
                return controller_opt.resources.hugepage_limits().as_ref();
            }
        }

        None
    }
}

impl StatsProvider for HugeTlb {
    type Stats = HashMap<String, HugeTlbStats>;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let page_sizes = supported_page_sizes()?;
        let mut hugetlb_stats = HashMap::with_capacity(page_sizes.len());

        for page_size in &page_sizes {
            let stats = Self::stats_for_page_size(cgroup_path, page_size)?;
            hugetlb_stats.insert(page_size.to_owned(), stats);
        }

        Ok(hugetlb_stats)
    }
}

impl HugeTlb {
    fn apply(root_path: &Path, hugetlb: &LinuxHugepageLimit) -> Result<()> {
        let page_size: String = hugetlb
            .page_size()
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
        let page_size: u64 = page_size.parse()?;
        if !Self::is_power_of_two(page_size) {
            bail!("page size must be in the format of 2^(integer)");
        }

        common::write_cgroup_file(
            root_path.join(format!("hugetlb.{}.limit_in_bytes", hugetlb.page_size())),
            hugetlb.limit(),
        )?;
        Ok(())
    }

    fn is_power_of_two(number: u64) -> bool {
        (number != 0) && (number & (number - 1)) == 0
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
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::LinuxHugepageLimit;
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
        HugeTlb::apply(&tmp, &hugetlb).expect("apply hugetlb");
        let content = read_to_string(tmp.join(page_file_name)).expect("Read hugetlb file content");
        assert_eq!(hugetlb.limit().to_string(), content);
    }

    #[test]
    fn test_set_hugetlb_with_invalid_page_size() {
        let tmp = create_temp_dir("test_set_hugetlb_with_invalid_page_size")
            .expect("create temp directory for test");

        let hugetlb = LinuxHugepageLimit {
            page_size: "3MB".to_owned(),
            limit: 16384,
        };

        let result = HugeTlb::apply(&tmp, &hugetlb);
        assert!(
            result.is_err(),
            "page size that is not a power of two should be an error"
        );
    }

    quickcheck! {
        fn property_test_set_hugetlb(hugetlb: LinuxHugepageLimit) -> bool {
            let page_file_name = format!("hugetlb.{:?}.limit_in_bytes", hugetlb.page_size());
            let tmp = create_temp_dir("property_test_set_hugetlb").expect("create temp directory for test");
            set_fixture(&tmp, &page_file_name, "0").expect("Set fixture for page size");

            let result = HugeTlb::apply(&tmp, &hugetlb);

            let page_size: String = hugetlb
            .page_size()
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
            let page_size: u64 = page_size.parse().expect("parse page size");

            if HugeTlb::is_power_of_two(page_size) && page_size != 1 {
                let content =
                    read_to_string(tmp.join(page_file_name)).expect("Read hugetlb file content");
                hugetlb.limit().to_string() == content
            } else {
                result.is_err()
            }
        }
    }

    #[test]
    fn test_stat_hugetlb() {
        let tmp = create_temp_dir("test_stat_hugetlb").expect("create temp directory for test");
        set_fixture(&tmp, "hugetlb.2MB.usage_in_bytes", "1024\n").expect("set hugetlb usage");
        set_fixture(&tmp, "hugetlb.2MB.max_usage_in_bytes", "4096\n")
            .expect("set hugetlb max usage");
        set_fixture(&tmp, "hugetlb.2MB.failcnt", "5").expect("set hugetlb fail count");

        let actual = HugeTlb::stats_for_page_size(&tmp, "2MB").expect("get cgroup stats");

        let expected = HugeTlbStats {
            usage: 1024,
            max_usage: 4096,
            fail_count: 5,
        };
        assert_eq!(actual, expected);
    }
}
