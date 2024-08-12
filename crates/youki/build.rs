use anyhow::Result;
use vergen_gitcl::{Emitter, GitclBuilder};

pub fn main() -> Result<()> {
    if Emitter::default()
        .add_instructions(&GitclBuilder::all_git()?)?
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
