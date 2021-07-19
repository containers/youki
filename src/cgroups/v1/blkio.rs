use std::path::Path;

use crate::cgroups::{common, v1::Controller};
use anyhow::Result;
use async_trait::async_trait;
use oci_spec::{LinuxBlockIo, LinuxResources};
use rio::Rio;

const CGROUP_BLKIO_THROTTLE_READ_BPS: &str = "blkio.throttle.read_bps_device";
const CGROUP_BLKIO_THROTTLE_WRITE_BPS: &str = "blkio.throttle.write_bps_device";
const CGROUP_BLKIO_THROTTLE_READ_IOPS: &str = "blkio.throttle.read_iops_device";
const CGROUP_BLKIO_THROTTLE_WRITE_IOPS: &str = "blkio.throttle.write_iops_device";

pub struct Blkio {}

#[async_trait]
impl Controller for Blkio {
    type Resource = LinuxBlockIo;

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply blkio cgroup config");

        if let Some(blkio) = Self::needs_to_handle(linux_resources) {
            Self::apply(ring, cgroup_root, blkio).await?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(blkio) = &linux_resources.block_io {
            return Some(blkio);
        }

        None
    }
}

impl Blkio {
    async fn apply(ring: &Rio, root_path: &Path, blkio: &LinuxBlockIo) -> Result<()> {
        let trbd_file = common::open_cgroup_file(&root_path.join(CGROUP_BLKIO_THROTTLE_READ_BPS));
        for trbd in &blkio.blkio_throttle_read_bps_device {
            common::async_write_cgroup_file(
                ring,
                &trbd_file,
                &format!("{}:{} {}", trbd.major, trbd.minor, trbd.rate),
            ).await?;
        }

        let twbd_file = common::open_cgroup_file(&root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_BPS));
        for twbd in &blkio.blkio_throttle_write_bps_device {
            common::async_write_cgroup_file(
                ring,
                &twbd_file,
                &format!("{}:{} {}", twbd.major, twbd.minor, twbd.rate),
            ).await?;
        }

        let trid_file = common::open_cgroup_file(&root_path.join(CGROUP_BLKIO_THROTTLE_READ_IOPS));
        for trid in &blkio.blkio_throttle_read_iops_device {
            common::async_write_cgroup_file(
                ring,
                &trid_file,
                &format!("{}:{} {}", trid.major, trid.minor, trid.rate),
            ).await?;
        }

        let twid_file = common::open_cgroup_file(&root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_IOPS));
        for twid in &blkio.blkio_throttle_write_iops_device {
            common::async_write_cgroup_file(
                ring,
                &twid_file,
                &format!("{}:{} {}", twid.major, twid.minor, twid.rate),
            ).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cgroups::test::{setup, aw};
    use oci_spec::{LinuxBlockIo, LinuxThrottleDevice};

    struct BlockIoBuilder {
        block_io: LinuxBlockIo,
    }

    impl BlockIoBuilder {
        fn new() -> Self {
            let block_io = LinuxBlockIo {
                blkio_weight: Some(0),
                blkio_leaf_weight: Some(0),
                blkio_weight_device: vec![],
                blkio_throttle_read_bps_device: vec![],
                blkio_throttle_write_bps_device: vec![],
                blkio_throttle_read_iops_device: vec![],
                blkio_throttle_write_iops_device: vec![],
            };

            Self { block_io }
        }

        fn with_read_bps(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_read_bps_device = throttle;
            self
        }

        fn with_write_bps(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_write_bps_device = throttle;
            self
        }

        fn with_read_iops(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_read_iops_device = throttle;
            self
        }

        fn with_write_iops(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_write_iops_device = throttle;
            self
        }

        fn build(self) -> LinuxBlockIo {
            self.block_io
        }
    }

    #[test]
    fn test_set_blkio_read_bps() {
        let (tmp, throttle) = setup("test_set_blkio_read_bps", CGROUP_BLKIO_THROTTLE_READ_BPS);

        let blkio = BlockIoBuilder::new()
            .with_read_bps(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        let ring = rio::new().expect("start io_uring");
        aw!(Blkio::apply(&ring, &tmp, &blkio)).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", CGROUP_BLKIO_THROTTLE_READ_BPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_write_bps() {
        let (tmp, throttle) = setup("test_set_blkio_write_bps", CGROUP_BLKIO_THROTTLE_WRITE_BPS);

        let blkio = BlockIoBuilder::new()
            .with_write_bps(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        let ring = rio::new().expect("start io_uring");
        aw!(Blkio::apply(&ring, &tmp, &blkio)).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", CGROUP_BLKIO_THROTTLE_WRITE_BPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_read_iops() {
        let (tmp, throttle) = setup("test_set_blkio_read_iops", CGROUP_BLKIO_THROTTLE_READ_IOPS);

        let blkio = BlockIoBuilder::new()
            .with_read_iops(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        let ring = rio::new().expect("start io_uring");
        aw!(Blkio::apply(&ring, &tmp, &blkio)).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", CGROUP_BLKIO_THROTTLE_READ_IOPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_write_iops() {
        let (tmp, throttle) = setup(
            "test_set_blkio_write_iops",
            CGROUP_BLKIO_THROTTLE_WRITE_IOPS,
        );

        let blkio = BlockIoBuilder::new()
            .with_write_iops(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        let ring = rio::new().expect("start io_uring");
        aw!(Blkio::apply(&ring, &tmp, &blkio)).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", CGROUP_BLKIO_THROTTLE_WRITE_IOPS));

        assert_eq!("8:0 102400", content);
    }
}
