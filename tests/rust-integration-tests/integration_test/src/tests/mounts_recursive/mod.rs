use crate::utils::test_inside_container;
use nix::mount::{mount, umount, MsFlags};
use oci_spec::runtime::{
    get_default_mounts, LinuxBuilder, Mount, ProcessBuilder, Spec, SpecBuilder,
};
use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use test_framework::{Test, TestGroup, TestResult};

fn get_spec(added_mounts: Vec<Mount>) -> Spec {
    let mut mounts = get_default_mounts();
    for mount in added_mounts {
        mounts.push(mount);
    }

    SpecBuilder::default()
        .mounts(mounts)
        .linux(
            // Need to reset the read-only paths
            LinuxBuilder::default()
                .readonly_paths(vec![])
                .build()
                .expect("error in building linux config"),
        )
        .process(
            ProcessBuilder::default()
                .args(vec![
                    "runtimetest".to_string(),
                    "mounts_recursive".to_string(),
                ])
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
}

fn setup_mount(mount_dir: &Path, sub_mount_dir: &Path) {
    fs::create_dir(mount_dir).unwrap();
    mount::<Path, Path, str, str>(None, mount_dir, Some("tmpfs"), MsFlags::empty(), None).unwrap();
    fs::create_dir(sub_mount_dir).unwrap();
    mount::<Path, Path, str, str>(None, sub_mount_dir, Some("tmpfs"), MsFlags::empty(), None)
        .unwrap();
}

fn clean_mount(mount_dir: &Path, sub_mount_dir: &Path) {
    umount(sub_mount_dir).unwrap();
    umount(mount_dir).unwrap();
    fs::remove_dir_all(mount_dir).unwrap();
}

fn check_recursive_readonly() -> TestResult {
    let rro_test_base_dir = PathBuf::from_str("/tmp").unwrap();
    let rro_dir_path = rro_test_base_dir.join("rro_dir");
    let rro_subdir_path = rro_dir_path.join("rro_subdir");
    let mount_dest_path = PathBuf::from_str("/mnt").unwrap();

    let mount_options = vec!["rbind".to_string(), "rro".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path.clone())
        .set_typ(None)
        .set_source(Some(rro_dir_path.clone()))
        .set_options(Some(mount_options.clone()));
    let spec = get_spec(vec![mount_spec]);

    let result = test_inside_container(spec, &|_| {
        setup_mount(&rro_dir_path, &rro_subdir_path);
        Ok(())
    });

    clean_mount(&rro_dir_path, &rro_subdir_path);

    result
}

fn check_recursive_nosuid() -> TestResult {
    let rnosuid_test_base_dir = PathBuf::from_str("/tmp").unwrap();
    let rnosuid_dir_path = rnosuid_test_base_dir.join("rnosuid_dir");
    let rnosuid_subdir_path = rnosuid_dir_path.join("rnosuid_subdir");
    let mount_dest_path = PathBuf::from_str("/mnt").unwrap();

    let mount_options = vec!["rbind".to_string(), "rnosuid".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path.clone())
        .set_typ(None)
        .set_source(Some(rnosuid_dir_path.clone()))
        .set_options(Some(mount_options.clone()));
    let spec = get_spec(vec![mount_spec]);

    let result = test_inside_container(spec, &|bundle_path| {
        setup_mount(&rnosuid_dir_path, &rnosuid_subdir_path);

        let executable_file_path = bundle_path.join("bin/yes");
        let in_container_executable_file_path = rnosuid_dir_path.join("executable_file");
        let in_container_executable_subdir_file_path = rnosuid_subdir_path.join("executable_file");

        fs::copy(&executable_file_path, &in_container_executable_file_path)?;
        fs::copy(
            &executable_file_path,
            &in_container_executable_subdir_file_path,
        )?;

        let in_container_executable_file = fs::File::open(&in_container_executable_file_path)?;
        let in_container_executable_subdir_file =
            fs::File::open(&in_container_executable_subdir_file_path)?;

        let mut in_container_executable_file_perm =
            in_container_executable_file.metadata()?.permissions();
        let mut in_container_executable_subdir_file_perm = in_container_executable_subdir_file
            .metadata()?
            .permissions();

        in_container_executable_file_perm
            .set_mode(in_container_executable_file_perm.mode() + 0o4000);
        in_container_executable_subdir_file_perm
            .set_mode(in_container_executable_subdir_file_perm.mode() + 0o4000);

        in_container_executable_file.set_permissions(in_container_executable_file_perm.clone())?;
        in_container_executable_subdir_file
            .set_permissions(in_container_executable_subdir_file_perm.clone())?;

        Ok(())
    });

    clean_mount(&rnosuid_dir_path, &rnosuid_subdir_path);

    result
}

pub fn get_mounts_recursive_test() -> TestGroup {
    let rro_test = Test::new("rro_test", Box::new(check_recursive_readonly));
    let rnosuid_test = Test::new("rnosuid_test", Box::new(check_recursive_nosuid));

    let mut tg = TestGroup::new("mounts_recursive");
    tg.add(vec![Box::new(rro_test), Box::new(rnosuid_test)]);

    tg
}
