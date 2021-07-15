use flate2::read::GzDecoder;
use rand::Rng;
use std::fs::File;
use std::io;
use std::{env, fs, path::PathBuf};
use tar::Archive;
use testanything::tap_suite_builder::TapSuiteBuilder;
use testanything::{tap_test::TapTest, tap_test_builder::TapTestBuilder};
use uuid::Uuid;

pub fn initialize_test(project_path: &PathBuf) -> Result<(), std::io::Error> {
    let result = prepare_test_workspace(project_path);
    return result;
}

pub fn cleanup_test(project_path: &PathBuf) -> Result<(), std::io::Error> {
    let result = delete_test_workspace(project_path);
    return result;
}

pub fn create_project_path() -> PathBuf {
    let current_dir_path_result = env::current_dir();
    return match current_dir_path_result {
        Ok(path_buf) => path_buf,
        Err(_) => panic!("directory is not found"),
    };
}

// This will generate the UUID needed when creating the container.
pub fn generate_uuid() -> Uuid {
    let mut rng = rand::thread_rng();
    const CHARSET: &[u8] = b"0123456789abcdefABCDEF";

    let rand_string: String = (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    return match Uuid::parse_str(&rand_string) {
        Ok(uuid) => uuid,
        Err(e) => panic!("{}", e),
    };
}

pub fn test_builder(status: bool, name: &str, diagnostic: &str) -> TapTest {
    TapTestBuilder::new()
        .name(name)
        .passed(status)
        .diagnostics(&vec![diagnostic])
        .finalize()
}

pub fn print_test_results(test_name: &str, tests: Vec<TapTest>) -> () {
    let tap_suite = TapSuiteBuilder::new()
        .name(test_name)
        .tests(tests)
        .finalize();

    // print to stdout
    println!("# Start {}", test_name);
    match tap_suite.print(io::stdout().lock()) {
        Ok(_) => {}
        Err(reason) => eprintln!("{}", reason),
    }
    println!("\n# End {}", test_name);
}

// Temporary files to be used for testing are created in the `integration-workspace`.
fn prepare_test_workspace(project_path: &PathBuf) -> Result<(), std::io::Error> {
    let integration_test_workspace_path = project_path.join("integration-workspace");
    let create_dir_result = fs::create_dir_all(&integration_test_workspace_path);
    if fs::create_dir_all(&integration_test_workspace_path).is_err() {
        return create_dir_result;
    }
    let tar_file_name = "bundle.tar.gz";
    let tar_path = integration_test_workspace_path.join(tar_file_name);
    fs::copy(
        tar_file_name,
        &integration_test_workspace_path.join(tar_file_name),
    )?;
    let tar_gz = File::open(tar_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(integration_test_workspace_path)?;

    Ok(())
}

// This deletes all temporary files.
fn delete_test_workspace(project_path: &PathBuf) -> Result<(), std::io::Error> {
    fs::remove_dir_all(project_path.join("integration-workspace"))?;

    Ok(())
}
