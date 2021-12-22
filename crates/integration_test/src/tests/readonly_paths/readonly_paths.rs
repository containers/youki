use crate::utils::test_inside_container;
use anyhow::bail;
use oci_spec::runtime::LinuxBuilder;
use oci_spec::runtime::{Spec, SpecBuilder};
use std::path::PathBuf;
use test_framework::{Test, TestGroup, TestResult};

fn get_spec(readonly_paths: Vec<String>) -> Spec {
    SpecBuilder::default()
        .linux(
            LinuxBuilder::default()
                .readonly_paths(readonly_paths)
                .build()
                .expect("could not build"),
        )
        .build()
        .unwrap()
}

fn check_readonly_paths() -> TestResult {
    // here we abbreviate 'readonly' as ro for variable names,
    // purely for ease of writing

    let ro_dir = "readonly_dir";
    let ro_subdir = "readonly_subdir";
    let ro_file = "readonly_file";

    // in the runtime-tools tests, they start these with a '/',
    // but in that case, when joined with any path later,
    // the '/' takes preference, and path is not actually joined
    // eg : (test).join(t1) = test/t1
    //      (test).join(.t1) = /t1
    // which is not what we want, so we leave them without '/'
    let ro_dir_top = PathBuf::from(ro_dir);
    let ro_file_top = PathBuf::from(ro_file);

    let ro_dir_sub = ro_dir_top.join(ro_subdir);
    let ro_file_sub = ro_dir_top.join(ro_file);
    let ro_file_sub_sub = ro_dir_sub.join(ro_file);

    let root = PathBuf::from("/");

    let ro_paths = vec![
        root.join(&ro_dir_top).to_string_lossy().to_string(),
        root.join(&ro_file_top).to_string_lossy().to_string(),
        root.join(&ro_dir_sub).to_string_lossy().to_string(),
        root.join(&ro_file_sub).to_string_lossy().to_string(),
        root.join(&ro_file_sub_sub).to_string_lossy().to_string(),
    ];

    let spec = get_spec(ro_paths);
    test_inside_container(spec, &|bundle| {
        // use std::{fs, io};
        // let bundle_path = bundle.as_ref();
        // let test_dir = bundle_path.join(&ro_dir_sub);

        // match fs::create_dir_all(&test_dir) {
        //     io::Result::Ok(_) => { /*This is expected*/ }
        //     io::Result::Err(e) => {
        //         bail!(e)
        //     }
        // }

        // match fs::File::create(test_dir.join("tmp")) {
        //     io::Result::Ok(_) => { /*This is expected*/ }
        //     io::Result::Err(e) => {
        //         bail!(e)
        //     }
        // }

        // let test_sub_sub_file = bundle_path.join(&ro_file_sub_sub);
        // match fs::File::create(&test_sub_sub_file) {
        //     io::Result::Ok(_) => { /*This is expected*/ }
        //     io::Result::Err(e) => {
        //         bail!(e)
        //     }
        // }

        // let test_sub_file = bundle_path.join(&ro_file_sub);
        // match fs::File::create(&test_sub_file) {
        //     io::Result::Ok(_) => { /*This is expected*/ }
        //     io::Result::Err(e) => {
        //         bail!(e)
        //     }
        // }

        // let test_file = bundle_path.join(&ro_file);
        // match fs::File::create(&test_file) {
        //     io::Result::Ok(_) => { /*This is expected*/ }
        //     io::Result::Err(e) => {
        //         bail!(e)
        //     }
        // }

        Ok(())
    })
}

pub fn get_ro_paths_test<'a>() -> TestGroup<'a> {
    let ro_paths = Test::new("readonly_paths", Box::new(check_readonly_paths));
    let mut tg = TestGroup::new("readonly_paths");
    tg.add(vec![Box::new(ro_paths)]);
    tg
}
