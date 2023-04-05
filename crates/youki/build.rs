use anyhow::Result;
use vergen::EmitBuilder;

fn main() -> Result<()> {
    if EmitBuilder::builder()
        .fail_on_error()
        .git_sha(true)
        .emit()
        .is_err()
    {
        // currently we only inject git sha, so just this
        // else we will need to think of more elegant way to check
        // what failed, and what needs to be added
        println!("cargo:rustc-env=VERGEN_GIT_SHA=unknown");
    }
    Ok(())
}
