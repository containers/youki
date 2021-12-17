use anyhow::Result;
use vergen::{vergen, Config, ShaKind};

fn main() -> Result<()> {
    let mut config = Config::default();
    *config.git_mut().sha_kind_mut() = ShaKind::Short;
    vergen(config)
}
