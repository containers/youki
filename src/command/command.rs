use std::path::Path;

use anyhow::Result;

pub trait Command {
    fn pivot_rootfs(&self, path: &Path) -> Result<()>;
}
