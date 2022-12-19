use crate::utils::test_inside_container;
use nix::mount::{mount, umount, MsFlags};
use oci_spec::runtime::{get_default_mounts, Mount, ProcessBuilder, Spec, SpecBuilder};
use std::fs;
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

pub fn get_mounts_recursive_test() -> TestGroup {
    let rro_test = Test::new("rro_test", Box::new(check_recursive_readonly));
    let mut tg = TestGroup::new("mounts_recursive");
    tg.add(vec![Box::new(rro_test)]);
    tg
}
