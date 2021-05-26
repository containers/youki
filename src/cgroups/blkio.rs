use std::path::Path;


use async_trait::async_trait;
use smol::{fs::{OpenOptions, create_dir_all}, io::AsyncWriteExt};

use crate::{
    cgroups::Controller,
    spec::{LinuxBlockIo, LinuxResources},
};

const CGROUP_BLKIO_THROTTLE_READ_BPS: &str = "blkio.throttle.read_bps_device";
const CGROUP_BLKIO_THROTTLE_WRITE_BPS: &str = "blkio.throttle.write_bps_device";
const CGROUP_BLKIO_THROTTLE_READ_IOPS: &str = "blkio.throttle.read_iops_device";
const CGROUP_BLKIO_THROTTLE_WRITE_IOPS: &str = "blkio.throttle.write_iops_device";

pub struct Blkio {}

#[async_trait]
impl Controller for Blkio {
    async fn apply(
        linux_resources: &LinuxResources,
        cgroup_root: &Path,
        pid: nix::unistd::Pid,
    ) -> anyhow::Result<()> {
        match &linux_resources.block_io {
            None => return Ok(()),
            Some(block_io) => {
                create_dir_all(cgroup_root).await?;
                Self::apply(cgroup_root, block_io).await?;
            }
        }

        let mut file = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("cgroup.procs")).await?;

        file.write_all(pid.to_string().as_bytes()).await?;
        file.sync_data().await?;

        Ok(())
    }
}

impl Blkio {
    async fn apply(root_path: &Path, blkio: &LinuxBlockIo) -> anyhow::Result<()> {
        for trbd in &blkio.blkio_throttle_read_bps_device {
            Self::write_file(
                &root_path.join(CGROUP_BLKIO_THROTTLE_READ_BPS),
                &format!("{}:{} {}", trbd.major, trbd.minor, trbd.rate),
            ).await?;
        }

        for twbd in &blkio.blkio_throttle_write_bps_device {
            Self::write_file(
                &root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_BPS),
                &format!("{}:{} {}", twbd.major, twbd.minor, twbd.rate),
            ).await?;
        }

        for trid in &blkio.blkio_throttle_read_iops_device {
            Self::write_file(
                &root_path.join(CGROUP_BLKIO_THROTTLE_READ_IOPS),
                &format!("{}:{} {}", trid.major, trid.minor, trid.rate),
            ).await?;
        }

        for twid in &blkio.blkio_throttle_write_iops_device {
            Self::write_file(
                &root_path.join(CGROUP_BLKIO_THROTTLE_WRITE_IOPS),
                &format!("{}:{} {}", twid.major, twid.minor, twid.rate),
            ).await?;
        }

        Ok(())
    }

    async fn write_file(file_path: &Path, data: &str) -> anyhow::Result<()> {
        let mut file = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(file_path).await?;

        file.write_all(data.as_bytes()).await?;
        file.sync_data().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::spec::{LinuxBlockIo, LinuxThrottleDevice};
    use std::io::Write;

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

    fn setup(testname: &str, throttle_type: &str) -> (PathBuf, PathBuf) {
        let tmp = create_temp_dir(testname).expect("create temp directory for test");
        let throttle_file = set_fixture(&tmp, throttle_type, "")
            .expect(&format!("set fixture for {}", throttle_type));

        (tmp, throttle_file)
    }

    fn set_fixture(
        temp_dir: &std::path::Path,
        filename: &str,
        val: &str,
    ) -> anyhow::Result<PathBuf> {
        let full_path = temp_dir.join(filename);

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&full_path)?;
            
        file.write_all(val.as_bytes())?;
        file.sync_data()?;

        Ok(full_path)
    }

    fn create_temp_dir(test_name: &str) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_set_blkio_read_bps() {
        let (test_root, throttle) =
            setup("test_set_blkio_read_bps", CGROUP_BLKIO_THROTTLE_READ_BPS);

        smol::block_on(async {
            let blkio = BlockIoBuilder::new()
                .with_read_bps(vec![LinuxThrottleDevice {
                    major: 8,
                    minor: 0,
                    rate: 102400,
                }])
                .build();

            Blkio::apply(&test_root, &blkio).await.expect("apply blkio");
            let content = std::fs::read_to_string(throttle)
                .expect(&format!("read {} content", CGROUP_BLKIO_THROTTLE_READ_BPS));

            assert_eq!("8:0 102400", content);
        });
    }

    #[test]
    fn test_set_blkio_write_bps() {
        let (test_root, throttle) =
            setup("test_set_blkio_write_bps", CGROUP_BLKIO_THROTTLE_WRITE_BPS);

        smol::block_on(async {
            let blkio = BlockIoBuilder::new()
                .with_write_bps(vec![LinuxThrottleDevice {
                    major: 8,
                    minor: 0,
                    rate: 102400,
                }])
                .build();

            Blkio::apply(&test_root, &blkio).await.expect("apply blkio");
            let content = std::fs::read_to_string(throttle)
                .expect(&format!("read {} content", CGROUP_BLKIO_THROTTLE_WRITE_BPS));

            assert_eq!("8:0 102400", content);
        });
    }

    #[test]
    fn test_set_blkio_read_iops() {
        let (test_root, throttle) =
            setup("test_set_blkio_read_iops", CGROUP_BLKIO_THROTTLE_READ_IOPS);

        smol::block_on(async {
            let blkio = BlockIoBuilder::new()
                .with_read_iops(vec![LinuxThrottleDevice {
                    major: 8,
                    minor: 0,
                    rate: 102400,
                }])
                .build();

            Blkio::apply(&test_root, &blkio).await.expect("apply blkio");
            let content = std::fs::read_to_string(throttle)
                .expect(&format!("read {} content", CGROUP_BLKIO_THROTTLE_READ_IOPS));

            assert_eq!("8:0 102400", content);
        });
    }

    #[test]
    fn test_set_blkio_write_iops() {
        let (test_root, throttle) = setup(
            "test_set_blkio_write_iops",
            CGROUP_BLKIO_THROTTLE_WRITE_IOPS,
        );

        smol::block_on(async {
            let blkio = BlockIoBuilder::new()
                .with_write_iops(vec![LinuxThrottleDevice {
                    major: 8,
                    minor: 0,
                    rate: 102400,
                }])
                .build();

            Blkio::apply(&test_root, &blkio).await.expect("apply blkio");
            let content = std::fs::read_to_string(throttle).expect(&format!(
                "read {} content",
                CGROUP_BLKIO_THROTTLE_WRITE_IOPS
            ));

            assert_eq!("8:0 102400", content);
        });
    }
}
