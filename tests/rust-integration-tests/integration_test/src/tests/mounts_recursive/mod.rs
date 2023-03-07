use crate::utils::test_inside_container;
use nix::mount::{mount, umount, MsFlags};
use nix::sys::stat::Mode;
use nix::unistd::{chown, Uid};
use oci_spec::runtime::{
    get_default_mounts, Capability, LinuxBuilder, LinuxCapabilitiesBuilder, Mount, ProcessBuilder,
    Spec, SpecBuilder,
};
use std::collections::hash_set::HashSet;
use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use test_framework::{Test, TestGroup, TestResult};

fn get_spec(added_mounts: Vec<Mount>, process_args: Vec<String>) -> Spec {
    let mut mounts = get_default_mounts();
    for mount in added_mounts {
        mounts.push(mount);
    }

    let caps = vec![
        Capability::Chown,
        Capability::DacOverride,
        Capability::Fsetid,
        Capability::Fowner,
        Capability::Mknod,
        Capability::NetRaw,
        Capability::Setgid,
        Capability::Setuid,
        Capability::Setfcap,
        Capability::Setpcap,
        Capability::NetBindService,
        Capability::SysChroot,
        Capability::Kill,
        Capability::AuditWrite,
    ];
    let mut cap_bounding = HashSet::new();
    let mut cap_effective = HashSet::new();
    let mut cap_permitted = HashSet::new();

    for cap in caps {
        cap_bounding.insert(cap);
        cap_effective.insert(cap);
        cap_permitted.insert(cap);
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
                .args(process_args)
                .capabilities(
                    LinuxCapabilitiesBuilder::default()
                        .bounding(cap_bounding)
                        .effective(cap_effective)
                        .permitted(cap_permitted)
                        .build()
                        .unwrap(),
                )
                .rlimits(vec![])
                .no_new_privileges(false)
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
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rro_dir_path.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

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
    let executable_file_name = "whoami";

    let mount_options = vec!["rbind".to_string(), "rnosuid".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path.clone())
        .set_typ(None)
        .set_source(Some(rnosuid_dir_path.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "{}; {}",
                mount_dest_path.join(executable_file_name).to_str().unwrap(),
                mount_dest_path
                    .join("rnosuid_subdir/whoami")
                    .to_str()
                    .unwrap()
            ),
        ],
    );

    let result = test_inside_container(spec, &|bundle_path| {
        setup_mount(&rnosuid_dir_path, &rnosuid_subdir_path);

        let executable_file_path = bundle_path.join("bin").join(executable_file_name);
        let in_container_executable_file_path = rnosuid_dir_path.join(executable_file_name);
        let in_container_executable_subdir_file_path =
            rnosuid_subdir_path.join(executable_file_name);

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

        // Change file user to nonexistent uid and set suid.
        // if rnosuid is applied, whoami command is executed as root.
        // but if not adapted, whoami command is executed as uid 1200 and make an error.
        chown(
            &in_container_executable_file_path,
            Some(Uid::from_raw(1200)),
            None,
        )
        .unwrap();
        chown(
            &in_container_executable_subdir_file_path,
            Some(Uid::from_raw(1200)),
            None,
        )
        .unwrap();
        in_container_executable_file_perm
            .set_mode(in_container_executable_file_perm.mode() | Mode::S_ISUID.bits());
        in_container_executable_subdir_file_perm
            .set_mode(in_container_executable_subdir_file_perm.mode() | Mode::S_ISUID.bits());

        in_container_executable_file.set_permissions(in_container_executable_file_perm.clone())?;
        in_container_executable_subdir_file
            .set_permissions(in_container_executable_subdir_file_perm.clone())?;

        Ok(())
    });

    clean_mount(&rnosuid_dir_path, &rnosuid_subdir_path);

    result
}

fn check_recursive_noexec() -> TestResult {
    let rnoexec_test_base_dir = PathBuf::from_str("/tmp").unwrap();
    let rnoexec_dir_path = rnoexec_test_base_dir.join("rnoexec_dir");
    let rnoexec_subdir_path = rnoexec_dir_path.join("rnoexec_subdir");
    let mount_dest_path = PathBuf::from_str("/mnt").unwrap();

    let mount_options = vec!["rbind".to_string(), "rnoexec".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rnoexec_dir_path.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

    let result = test_inside_container(spec, &|bundle_path| {
        setup_mount(&rnoexec_dir_path, &rnoexec_subdir_path);

        let executable_file_name = "echo";
        let executable_file_path = bundle_path.join("bin").join(executable_file_name);
        let in_container_executable_file_path = rnoexec_dir_path.join(executable_file_name);
        let in_container_executable_subdir_file_path =
            rnoexec_subdir_path.join(executable_file_name);

        fs::copy(&executable_file_path, in_container_executable_file_path)?;
        fs::copy(
            &executable_file_path,
            in_container_executable_subdir_file_path,
        )?;

        Ok(())
    });

    clean_mount(&rnoexec_dir_path, &rnoexec_subdir_path);

    result
}

/// rdiratime If set in attr_clr, removes the restriction that prevented updating access time for directories.
fn check_recursive_rdiratime() -> TestResult {
    let rdiratime_base_dir = PathBuf::from_str("/tmp/rdiratime").unwrap();
    let mount_dest_path = PathBuf::from_str("/rdiratime").unwrap();
    fs::create_dir(rdiratime_base_dir.clone()).unwrap();

    let mount_options = vec!["rbind".to_string(), "rdiratime".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rdiratime_base_dir.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

    let result = test_inside_container(spec, &|_| Ok(()));

    fs::remove_dir(rdiratime_base_dir).unwrap();
    result
}

/// If set in attr_set, prevents updating access time for directories on this mount
fn check_recursive_rnodiratime() -> TestResult {
    let rnodiratime_base_dir = PathBuf::from_str("/tmp/rnodiratime").unwrap();
    let mount_dest_path = PathBuf::from_str("/rnodiratime").unwrap();
    fs::create_dir(rnodiratime_base_dir.clone()).unwrap();

    let mount_options = vec!["rbind".to_string(), "rnodiratime".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rnodiratime_base_dir.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

    let result = test_inside_container(spec, &|_| Ok(()));
    fs::remove_dir(rnodiratime_base_dir).unwrap();
    result
}

fn check_recursive_rdev() -> TestResult {
    let rdev_base_dir = PathBuf::from_str("/dev").unwrap();
    let mount_dest_path = PathBuf::from_str("/rdev").unwrap();

    let mount_options = vec!["rbind".to_string(), "rdev".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rdev_base_dir.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

    let result = test_inside_container(spec, &|_| Ok(()));
    result
}

fn check_recursive_rnodev() -> TestResult {
    let rnodev_base_dir = PathBuf::from_str("/dev").unwrap();
    let mount_dest_path = PathBuf::from_str("/rnodev").unwrap();

    let mount_options = vec!["rbind".to_string(), "rnodev".to_string()];
    let mut mount_spec = Mount::default();
    mount_spec
        .set_destination(mount_dest_path)
        .set_typ(None)
        .set_source(Some(rnodev_base_dir.clone()))
        .set_options(Some(mount_options));
    let spec = get_spec(
        vec![mount_spec],
        vec!["runtimetest".to_string(), "mounts_recursive".to_string()],
    );

    let result = test_inside_container(spec, &|_| Ok(()));
    result
}

pub fn get_mounts_recursive_test() -> TestGroup {
    let rro_test = Test::new("rro_test", Box::new(check_recursive_readonly));
    let rnosuid_test = Test::new("rnosuid_test", Box::new(check_recursive_nosuid));
    let rnoexec_test = Test::new("rnoexec_test", Box::new(check_recursive_noexec));
    let rnodiratime_test = Test::new("rnodiratime_test", Box::new(check_recursive_rnodiratime));
    let rdiratime_test = Test::new("rdiratime_test", Box::new(check_recursive_rdiratime));
    let rdev_test = Test::new("rdev_test", Box::new(check_recursive_rdev));
    let rnodev_test = Test::new("rnodev_test", Box::new(check_recursive_rnodev));

    let mut tg = TestGroup::new("mounts_recursive");
    tg.add(vec![
        Box::new(rro_test),
        Box::new(rnosuid_test),
        Box::new(rnoexec_test),
        Box::new(rdiratime_test),
        Box::new(rnodiratime_test),
        Box::new(rdev_test),
        Box::new(rnodev_test),
    ]);

    tg
}
