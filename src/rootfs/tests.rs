mod rootfs {

    #[cfg(test)]
    mod tests {
        use crate::rootfs::*;
        use crate::syscall::test::{ChownArgs, MknodArgs, MountArgs, TestHelperSyscall};
        use crate::utils::TempDir;

        #[test]
        fn test_find_parent_mount() -> anyhow::Result<()> {
            let mount_infos = vec![
                MountInfo {
                    mnt_id: 11,
                    pid: 10,
                    majmin: "".to_string(),
                    root: "/".to_string(),
                    mount_point: PathBuf::from("/"),
                    mount_options: Default::default(),
                    opt_fields: vec![],
                    fs_type: "ext4".to_string(),
                    mount_source: Some("/dev/sda1".to_string()),
                    super_options: Default::default(),
                },
                MountInfo {
                    mnt_id: 12,
                    pid: 11,
                    majmin: "".to_string(),
                    root: "/".to_string(),
                    mount_point: PathBuf::from("/proc"),
                    mount_options: Default::default(),
                    opt_fields: vec![],
                    fs_type: "proc".to_string(),
                    mount_source: Some("proc".to_string()),
                    super_options: Default::default(),
                },
            ];

            let res = find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos)
                .context("Failed to get parent mount")?;
            assert_eq!(res.mnt_id, 11);
            Ok(())
        }

        #[test]
        fn test_find_parent_mount_with_empty_mount_infos() {
            let mount_infos = vec![];
            let res = find_parent_mount(Path::new("/path/to/rootfs"), &mount_infos);
            assert!(res.is_err());
        }

        #[test]
        fn test_to_sflag() {
            assert_eq!(
                SFlag::S_IFBLK | SFlag::S_IFCHR | SFlag::S_IFIFO,
                to_sflag(LinuxDeviceType::A)
            );
            assert_eq!(SFlag::S_IFBLK, to_sflag(LinuxDeviceType::B));
            assert_eq!(SFlag::S_IFCHR, to_sflag(LinuxDeviceType::C));
            assert_eq!(SFlag::S_IFCHR, to_sflag(LinuxDeviceType::U));
            assert_eq!(SFlag::S_IFIFO, to_sflag(LinuxDeviceType::P));
        }

        #[test]
        fn test_parse_mount() {
            assert_eq!(
                (MsFlags::empty(), "".to_string()),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/proc"))
                        .typ("proc")
                        .source(PathBuf::from("proc"))
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (MsFlags::MS_NOSUID, "mode=755,size=65536k".to_string()),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/dev"))
                        .typ("tmpfs")
                        .source(PathBuf::from("tmpfs"))
                        .options(vec![
                            "nosuid".to_string(),
                            "strictatime".to_string(),
                            "mode=755".to_string(),
                            "size=65536k".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (
                    MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                    "newinstance,ptmxmode=0666,mode=0620,gid=5".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/dev/pts"))
                        .typ("devpts")
                        .source(PathBuf::from("devpts"))
                        .options(vec![
                            "nosuid".to_string(),
                            "noexec".to_string(),
                            "newinstance".to_string(),
                            "ptmxmode=0666".to_string(),
                            "mode=0620".to_string(),
                            "gid=5".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (
                    MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                    "mode=1777,size=65536k".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/dev/shm"))
                        .typ("tmpfs")
                        .source(PathBuf::from("shm"))
                        .options(vec![
                            "nosuid".to_string(),
                            "noexec".to_string(),
                            "nodev".to_string(),
                            "mode=1777".to_string(),
                            "size=65536k".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (
                    MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                    "".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/dev/mqueue"))
                        .typ("mqueue")
                        .source(PathBuf::from("mqueue"))
                        .options(vec![
                            "nosuid".to_string(),
                            "noexec".to_string(),
                            "nodev".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (
                    MsFlags::MS_NOSUID
                        | MsFlags::MS_NOEXEC
                        | MsFlags::MS_NODEV
                        | MsFlags::MS_RDONLY,
                    "".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/sys"))
                        .typ("sysfs")
                        .source(PathBuf::from("sysfs"))
                        .options(vec![
                            "nosuid".to_string(),
                            "noexec".to_string(),
                            "nodev".to_string(),
                            "ro".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            assert_eq!(
                (
                    MsFlags::MS_NOSUID
                        | MsFlags::MS_NOEXEC
                        | MsFlags::MS_NODEV
                        | MsFlags::MS_RDONLY,
                    "".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .destination(PathBuf::from("/sys/fs/cgroup"))
                        .typ("cgroup")
                        .source(PathBuf::from("cgroup"))
                        .options(vec![
                            "nosuid".to_string(),
                            "noexec".to_string(),
                            "nodev".to_string(),
                            "relatime".to_string(),
                            "ro".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
            // this case is just for coverage purpose
            assert_eq!(
                (
                    MsFlags::MS_NOSUID
                        | MsFlags::MS_NODEV
                        | MsFlags::MS_NOEXEC
                        | MsFlags::MS_REMOUNT
                        | MsFlags::MS_DIRSYNC
                        | MsFlags::MS_NOATIME
                        | MsFlags::MS_NODIRATIME
                        | MsFlags::MS_BIND
                        | MsFlags::MS_UNBINDABLE,
                    "".to_string()
                ),
                parse_mount(
                    &MountBuilder::default()
                        .options(vec![
                            "defaults".to_string(),
                            "ro".to_string(),
                            "rw".to_string(),
                            "suid".to_string(),
                            "nosuid".to_string(),
                            "dev".to_string(),
                            "nodev".to_string(),
                            "exec".to_string(),
                            "noexec".to_string(),
                            "sync".to_string(),
                            "async".to_string(),
                            "dirsync".to_string(),
                            "remount".to_string(),
                            "mand".to_string(),
                            "nomand".to_string(),
                            "atime".to_string(),
                            "noatime".to_string(),
                            "diratime".to_string(),
                            "nodiratime".to_string(),
                            "bind".to_string(),
                            "rbind".to_string(),
                            "unbindable".to_string(),
                            "runbindable".to_string(),
                            "private".to_string(),
                            "rprivate".to_string(),
                            "shared".to_string(),
                            "rshared".to_string(),
                            "slave".to_string(),
                            "rslave".to_string(),
                            "relatime".to_string(),
                            "norelatime".to_string(),
                            "strictatime".to_string(),
                            "nostrictatime".to_string(),
                        ])
                        .build()
                        .unwrap()
                )
            );
        }

        #[test]
        fn test_setup_ptmx() {
            {
                let tmp_dir = TempDir::new("/tmp/test_setup_ptmx").unwrap();
                let rootfs = RootFS::new();
                assert!(rootfs.setup_ptmx(tmp_dir.path()).is_ok());
                let want = (PathBuf::from("pts/ptmx"), tmp_dir.path().join("dev/ptmx"));
                let got = &rootfs
                    .syscall
                    .as_any()
                    .downcast_ref::<TestHelperSyscall>()
                    .unwrap()
                    .get_symlink_args()[0];
                assert_eq!(want, *got)
            }
            // make remove_file goes into the bail! path
            {
                let tmp_dir = TempDir::new("/tmp/test_setup_ptmx").unwrap();
                open(
                    &tmp_dir.path().join("dev"),
                    OFlag::O_RDWR | OFlag::O_CREAT,
                    Mode::from_bits_truncate(0o644),
                )
                .unwrap();

                let rootfs = RootFS::new();
                assert!(rootfs.setup_ptmx(tmp_dir.path()).is_err());
                assert_eq!(
                    0,
                    rootfs
                        .syscall
                        .as_any()
                        .downcast_ref::<TestHelperSyscall>()
                        .unwrap()
                        .get_symlink_args()
                        .len()
                );
            }
        }

        #[test]
        fn test_setup_default_symlinks() {
            let tmp_dir = TempDir::new("/tmp/test_setup_default_symlinks").unwrap();
            let rootfs = RootFS::new();
            assert!(rootfs.setup_default_symlinks(tmp_dir.path()).is_ok());
            let want = vec![
                (
                    PathBuf::from("/proc/self/fd"),
                    tmp_dir.path().join("dev/fd"),
                ),
                (
                    PathBuf::from("/proc/self/fd/0"),
                    tmp_dir.path().join("dev/stdin"),
                ),
                (
                    PathBuf::from("/proc/self/fd/1"),
                    tmp_dir.path().join("dev/stdout"),
                ),
                (
                    PathBuf::from("/proc/self/fd/2"),
                    tmp_dir.path().join("dev/stderr"),
                ),
            ];
            let got = rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_symlink_args();
            assert_eq!(want, got)
        }

        #[test]
        fn test_bind_dev() {
            let tmp_dir = TempDir::new("/tmp/test_bind_dev").unwrap();
            let rootfs = RootFS::new();
            assert!(rootfs
                .bind_dev(
                    tmp_dir.path(),
                    &LinuxDeviceBuilder::default()
                        .path(PathBuf::from("/null"))
                        .build()
                        .unwrap(),
                )
                .is_ok());

            let want = MountArgs {
                source: Some(PathBuf::from("/null")),
                target: tmp_dir.path().join("null"),
                fstype: Some("bind".to_string()),
                flags: MsFlags::MS_BIND,
                data: None,
            };
            let got = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args()[0];
            assert_eq!(want, *got);
        }

        #[test]
        fn test_mknod_dev() {
            let tmp_dir = TempDir::new("/tmp/test_mknod_dev").unwrap();
            let rootfs = RootFS::new();
            assert!(rootfs
                .mknod_dev(
                    tmp_dir.path(),
                    &LinuxDeviceBuilder::default()
                        .path(PathBuf::from("/null"))
                        .major(1)
                        .minor(3)
                        .typ(LinuxDeviceType::C)
                        .file_mode(0o644u32)
                        .uid(1000u32)
                        .gid(1000u32)
                        .build()
                        .unwrap(),
                )
                .is_ok());

            let want_mknod = MknodArgs {
                path: tmp_dir.path().join("null"),
                kind: SFlag::S_IFCHR,
                perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
                dev: 259,
            };
            let got_mknod = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mknod_args()[0];
            assert_eq!(want_mknod, *got_mknod);

            let want_chown = ChownArgs {
                path: tmp_dir.path().join("null"),
                owner: Some(Uid::from_raw(1000)),
                group: Some(Gid::from_raw(1000)),
            };
            let got_chown = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_chown_args()[0];
            assert_eq!(want_chown, *got_chown);
        }

        #[test]
        fn test_create_devices() {
            let tmp_dir = TempDir::new("/tmp/test_create_devices").unwrap();
            let rootfs = RootFS::new();
            let devices = vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from("/dev/null"))
                .major(1)
                .minor(3)
                .typ(LinuxDeviceType::C)
                .file_mode(0o644u32)
                .uid(1000u32)
                .gid(1000u32)
                .build()
                .unwrap()];

            assert!(rootfs
                .create_devices(tmp_dir.path(), &devices, true)
                .is_ok());

            let want = MountArgs {
                source: Some(PathBuf::from("/dev/null")),
                target: tmp_dir.path().join("dev/null"),
                fstype: Some("bind".to_string()),
                flags: MsFlags::MS_BIND,
                data: None,
            };
            let got = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args()[0];
            assert_eq!(want, *got);

            assert!(rootfs
                .create_devices(tmp_dir.path(), &devices, false)
                .is_ok());

            let want = MknodArgs {
                path: tmp_dir.path().join("dev/null"),
                kind: SFlag::S_IFCHR,
                perm: Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH,
                dev: 259,
            };
            let got = &rootfs
                .syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mknod_args()[0];
            assert_eq!(want, *got);
        }

        #[test]
        fn test_mount_to_container() {
            let tmp_dir = TempDir::new("/tmp/test_mount_to_container").unwrap();
            let rootfs = RootFS::new();
            let mount = &MountBuilder::default()
                .destination(PathBuf::from("/dev/pts"))
                .typ("devpts")
                .source(PathBuf::from("devpts"))
                .options(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "newinstance".to_string(),
                    "ptmxmode=0666".to_string(),
                    "mode=0620".to_string(),
                    "gid=5".to_string(),
                ])
                .build()
                .unwrap();
            let (flags, data) = parse_mount(mount);

            assert!(rootfs
                .mount_to_container(mount, tmp_dir.path(), flags, &data, None)
                .is_ok());
        }
    }
}
