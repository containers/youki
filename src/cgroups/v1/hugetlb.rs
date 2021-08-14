use std::path::Path;

use anyhow::{bail, Result};
use async_trait::async_trait;
use rio::Rio;

use crate::cgroups::{common, v1::Controller};
use oci_spec::{LinuxHugepageLimit, LinuxResources};

pub struct Hugetlb {}

#[async_trait]
impl Controller for Hugetlb {
    type Resource = Vec<LinuxHugepageLimit>;

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &std::path::Path) -> Result<()> {
        log::debug!("Apply Hugetlb cgroup config");

        if let Some(hugepage_limits) = Self::needs_to_handle(linux_resources) {
            for hugetlb in hugepage_limits {
                Self::apply(ring, cgroup_root, hugetlb).await?
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

impl Hugetlb {
    async fn apply(ring: &Rio, root_path: &Path, hugetlb: &LinuxHugepageLimit) -> Result<()> {
        let page_size: String = hugetlb
            .page_size
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
        let page_size: u64 = page_size.parse()?;
        if !Self::is_power_of_two(page_size) {
            bail!("page size must be in the format of 2^(integer)");
        }

        let file = common::open_cgroup_file(root_path.join(format!("hugetlb.{}.limit_in_bytes", hugetlb.page_size)))?;
        common::async_write_cgroup_file(ring, &file, hugetlb.limit).await?;

        Ok(())
    }

    fn is_power_of_two(number: u64) -> bool {
        (number != 0) && (number & (number - 1)) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::{set_fixture, aw};
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
        let ring = rio::new().expect("start io_uring");
        aw!(Hugetlb::apply(&ring, &tmp, &hugetlb)).expect("apply hugetlb");
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

        let ring = rio::new().expect("start io_uring");
        let result = aw!(Hugetlb::apply(&ring, &tmp, &hugetlb));
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

            let ring = rio::new().expect("start io_uring");
            let result = aw!(Hugetlb::apply(&ring, &tmp, &hugetlb));

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
