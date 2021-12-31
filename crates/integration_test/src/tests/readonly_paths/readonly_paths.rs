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
        use std::{fs, io};
        let bundle_path = bundle.as_ref();
        let test_dir = bundle_path.join(&ro_dir_sub);

        match fs::create_dir_all(&test_dir) {
            io::Result::Ok(_) => { /*This is expected*/ }
            io::Result::Err(e) => {
                bail!(e)
            }
        }

        match fs::File::create(test_dir.join("tmp")) {
            io::Result::Ok(_) => { /*This is expected*/ }
            io::Result::Err(e) => {
                bail!(e)
            }
        }

        let test_sub_sub_file = bundle_path.join(&ro_file_sub_sub);
        match fs::File::create(&test_sub_sub_file) {
            io::Result::Ok(_) => { /*This is expected*/ }
            io::Result::Err(e) => {
                bail!(e)
            }
        }

        let test_sub_file = bundle_path.join(&ro_file_sub);
        match fs::File::create(&test_sub_file) {
            io::Result::Ok(_) => { /*This is expected*/ }
            io::Result::Err(e) => {
                bail!(e)
            }
        }

        let test_file = bundle_path.join(&ro_file);
        match fs::File::create(&test_file) {
            io::Result::Ok(_) => { /*This is expected*/ }
            io::Result::Err(e) => {
                bail!(e)
            }
        }

        Ok(())
    })
}

fn check_readonly_rel_path() -> TestResult {
    let ro_rel_path = "readonly_relpath";
    let ro_paths = vec![ro_rel_path.to_string()];
    let spec = get_spec(ro_paths);

    test_inside_container(spec, &|bundle| {
        use std::{fs, io};
        let bundle_path = bundle.as_ref();
        let test_file = bundle_path.join(ro_rel_path);

        match fs::metadata(&test_file) {
            io::Result::Ok(md) => {
                bail!(
                    "reading path {:?} should have given error, found {:?} instead",
                    test_file,
                    md
                )
            }
            io::Result::Err(e) => {
                let err = e.kind();
                if let io::ErrorKind::NotFound = err {
                    return Ok(());
                } else {
                    bail!("expected not found error, got {:?}", err);
                }
            }
        }
    })
}

fn check_readonly_symlinks() -> TestResult {
    let root = PathBuf::from("/");
    let ro_symlink = "readonly_symlink";
    let ro_paths = vec![root.join(&ro_symlink).to_string_lossy().to_string()];

    let spec = get_spec(ro_paths);

    test_inside_container(spec, &|bundle| {
        use std::{fs, io};
        let bundle_path = bundle.as_ref();
        let test_file = bundle_path.join(ro_symlink);

        match std::os::unix::fs::symlink("../readonly_symlink", &test_file) {
            io::Result::Ok(_) => { /* This is expected */ }
            io::Result::Err(e) => {
                bail!("error in creating symlink, to {:?} {:?}", test_file, e);
            }
        }
        let r_path = match fs::read_link(&test_file) {
            io::Result::Ok(p) => p,
            io::Result::Err(e) => {
                bail!("error in reading symlink at {:?} : {:?}", test_file, e);
            }
        };

        match fs::metadata(&r_path) {
            io::Result::Ok(md) => {
                bail!(
                    "reading symlink for {:?} should have given error, found {:?} instead",
                    test_file,
                    md
                )
            }
            io::Result::Err(e) => {
                let err = e.kind();
                if let io::ErrorKind::NotFound = err {
                    return Ok(());
                } else {
                    bail!("expected not found error, got {:?}", err);
                }
            }
        }
    })
}

fn test_node(mode: u32) -> TestResult {
    let root = PathBuf::from("/");
    let ro_device = "readonly_device";
    let ro_paths = vec![root.join(&ro_device).to_string_lossy().to_string()];

    let spec = get_spec(ro_paths);

    test_inside_container(spec, &|bundle| {
        use std::{fs, io};

        let bundle_path = bundle.as_ref();
        let test_file = bundle_path.join(&ro_device);
        // NOTE
        // yes, I know using unsafe willy-nilly is a bad idea,
        // especially given that OpenOptionsExt in std::os::unix::fs does provide a method
        // to set mode in open options, like this :

        // use std::os::unix::fs::OpenOptionsExt;
        // let mut opts = fs::OpenOptions::new();
        // opts.mode(mode);
        // opts.create(true);
        // if let io::Result::Err(e) = opts.open(&test_file) {
        //     bail!(
        //         "could not create device node at {:?} with mode {}, got error {:?}",
        //         test_file,
        //         mode ^ 0o666,
        //         e
        //     );
        // }

        // but that gives OsErr 22, invalid arguments.
        // That is why we directly use mknod from lib here

        let _path = test_file.to_string_lossy().as_ptr() as *const i8;
        let r = unsafe { libc::mknod(_path, mode, 0) };
        if r != 0 {
            bail!(
                "error in creating a device node at {:?} with mode {:?}, got return code {}",
                test_file,
                mode,
                r
            );
        }

        match fs::metadata(&test_file) {
            io::Result::Ok(_) => Ok(()),
            io::Result::Err(e) => {
                bail!("error in creating device node, {:?}", e)
            }
        }
    })
}

fn check_readonly_device_nodes() -> TestResult {
    let modes = [
        libc::S_IFBLK | 0o666,
        libc::S_IFCHR | 0o666,
        libc::S_IFIFO | 0o666,
    ];
    for mode in modes {
        let res = test_node(mode);
        if let TestResult::Failed(_) = res {
            return res;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    TestResult::Passed
}

pub fn get_ro_paths_test<'a>() -> TestGroup<'a> {
    let ro_paths = Test::new("readonly_paths", Box::new(check_readonly_paths));
    let ro_rel_paths = Test::new("readonly_rel_paths", Box::new(check_readonly_rel_path));
    let ro_symlinks = Test::new("readonly_symlinks", Box::new(check_readonly_symlinks));
    // let ro_device_nodes = Test::new(
    //     "readonly_device_nodes",
    //     Box::new(check_readonly_device_nodes),
    // );
    let ro_device_nodes_blk = Test::new(
        "readonly_device_nodes_blk",
        Box::new(|| test_node(libc::S_IFBLK | 0o666)),
    );
    let ro_device_nodes_chr = Test::new(
        "readonly_device_node_chr",
        Box::new(|| test_node(libc::S_IFCHR | 0o666)),
    );

    let ro_device_nodes_fifo = Test::new(
        "readonly_device_nodes_fifo",
        Box::new(|| test_node(libc::S_IFIFO | 0o666)),
    );
    let mut tg = TestGroup::new("readonly_paths");
    tg.add(vec![
        Box::new(ro_paths),
        Box::new(ro_rel_paths),
        Box::new(ro_symlinks),
        // Box::new(ro_device_nodes),
        Box::new(ro_device_nodes_blk),
        Box::new(ro_device_nodes_chr),
        Box::new(ro_device_nodes_fifo),
    ]);
    tg
}
