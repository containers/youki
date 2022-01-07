mod tests;
mod utils;

use oci_spec::runtime::Spec;
use std::path::PathBuf;

const SPEC_PATH: &'static str = "/config.json";

fn get_spec() -> Spec {
    let path = PathBuf::from(SPEC_PATH);
    match Spec::load(path) {
        Ok(spec) => spec,
        Err(e) => {
            eprintln!("Error in loading spec, {:?}", e);
            std::process::exit(66);
        }
    }
}

fn main() {
    let spec = get_spec();
    tests::validate_readonly_paths(&spec);
}
