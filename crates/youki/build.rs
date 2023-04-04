use anyhow::Result;
use vergen::EmitBuilder;

fn main() -> Result<()> {
    match EmitBuilder::builder().fail_on_error().git_sha(true).emit() {
        Ok(_) => {}
        Err(_e) => {
            println!("cargo:rustc-env=VERGEN_GIT_SHA=unknown");
        }
    }
    Ok(())
}
