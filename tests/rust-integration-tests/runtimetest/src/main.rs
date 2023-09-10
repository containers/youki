mod tests;
mod utils;

use oci_spec::runtime::Spec;
use std::env;
use std::path::PathBuf;

const SPEC_PATH: &str = "/config.json";

fn get_spec() -> Spec {
    let path = PathBuf::from(SPEC_PATH);
    match Spec::load(path) {
        Ok(spec) => spec,
        Err(e) => {
            eprintln!("Error in loading spec, {e:?}");
            std::process::exit(66);
        }
    }
}

fn main() {
    let spec = get_spec();
    let args: Vec<String> = env::args().collect();
    let execute_test = match args.get(1) {
        Some(execute_test) => execute_test.to_string(),
        None => return eprintln!("error due to execute test name not found"),
    };

    match &*execute_test {
        "readonly_paths" => tests::validate_readonly_paths(&spec),
        "mounts_recursive" => tests::validate_mounts_recursive(&spec),
        _ => eprintln!("error due to unexpected execute test name: {execute_test}"),
    }
}
