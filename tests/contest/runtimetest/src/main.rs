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

////////// ANCHOR: example_runtimetest_main
fn main() {
    let spec = get_spec();
    let args: Vec<String> = env::args().collect();
    let execute_test = match args.get(1) {
        Some(execute_test) => execute_test.to_string(),
        None => return eprintln!("error due to execute test name not found"),
    };

    match &*execute_test {
        "hello_world" => tests::hello_world(&spec),
        ////////// ANCHOR_END: example_runtimetest_main
        "readonly_paths" => tests::validate_readonly_paths(&spec),
        "set_host_name" => tests::validate_hostname(&spec),
        "mounts_recursive" => tests::validate_mounts_recursive(&spec),
        "domainname_test" => tests::validate_domainname(&spec),
        "seccomp" => tests::validate_seccomp(&spec),
        "sysctl" => tests::validate_sysctl(&spec),
        "scheduler_policy_other" => tests::validate_scheduler_policy(&spec),
        "scheduler_policy_batch" => tests::validate_scheduler_policy(&spec),
        _ => eprintln!("error due to unexpected execute test name: {execute_test}"),
    }
}
