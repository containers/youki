use anyhow::Result;
use vergen::{vergen, Config, ShaKind};

fn main() -> Result<()> {
    let mut config = Config::default();
    *config.git_mut().sha_kind_mut() = ShaKind::Short;
    *config.git_mut().skip_if_error_mut() = true;
    println!("cargo:rustc-env=VERGEN_GIT_SHA_SHORT=unknown");
    vergen(config)
}
