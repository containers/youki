# libcontainer

Youki crate for creating and managing cgroups.

### Init vs Tenant containers

Youki libcontainer will isolate sections of the system using Linux namespaces.

Use an **Init** container to create the first process in the new container isolated section of the system. After an init container is started, new **Tenant** container can be started along-side the recently created init process.

This allows for multiple processes to be started in the same isolation zone where they can share local network and storage as they share the same Linux namespaces.

### Building Container Bundle

Youki libcontainer requires [OCI](https://github.com/opencontainers/runtime-spec/blob/main/bundle.md) container bundle directory to start a new container.

Create a bundle directory from scratch.

```bash 
mkdir /var/lib/bundle/youki-bundle
cd /var/lib/bundle/youki-bundle

# Create busybox docker image
docker pull busybox
docker export youki-example | tar -C rootfs -xf -
docker rm busybox

youki spec # Or copy config.json
```

Now libcontainer can be used to manage the new container bundle directory.

```rust 
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::syscall::syscall::create_syscall;

ContainerBuilder::new("youki-example".to_owned(), create_syscall().as_ref())
 .with_root_path("/var/run/containers/youki").expect("invalid root path")
 .with_pid_file(Some("/var/run/docker.pid")).expect("invalid pid file")
 .with_console_socket(Some("/var/run/docker/sock.tty"))
 .as_init("/var/lib/bundle/youki-bundle")
 .build(); 
```

### Example libcontainer spec

The Youki spec can be generated with command `youki spec` and the `.process.args[0]` can be changed from `sh` to whatever command you want to run in your container.

If you see warnings such as

```
[WARN crates/libcgroups/src/v2/util.rs:41] 2023-01-21T15:17:11.303719086-08:00 Controller rdma is not yet implemented.
[WARN crates/libcgroups/src/v2/util.rs:41] 2023-01-21T15:17:11.303839233-08:00 Controller misc is not yet implemented.
[WARN crates/libcgroups/src/v2/util.rs:41] 2023-01-21T15:17:11.389348245-08:00 Controller rdma is not yet implemented.
[WARN crates/libcgroups/src/v2/util.rs:41] 2023-01-21T15:17:11.389361705-08:00 Controller misc is not yet implemented.
[WARN crates/libcontainer/src/process/container_init_process.rs:90] 2023-01-21T15:17:11.391740842-08:00 masked path "/proc/timer_stats" not exist
[WARN crates/libcontainer/src/process/container_init_process.rs:90] 2023-01-21T15:17:11.391763215-08:00 masked path "/proc/sched_debug" not exist
```

you can make changes to config.json to fix the warnings.

Example spec:

```json 
{
  "ociVersion": "1.0.2-dev",
  "root": {
    "path": "rootfs",
    "readonly": true
  },
  "mounts": [
    {
      "destination": "/proc",
      "type": "proc",
      "source": "proc"
    },
    {
      "destination": "/dev",
      "type": "tmpfs",
      "source": "tmpfs",
      "options": [
        "nosuid",
        "strictatime",
        "mode=755",
        "size=65536k"
      ]
    },
    {
      "destination": "/dev/pts",
      "type": "devpts",
      "source": "devpts",
      "options": [
        "nosuid",
        "noexec",
        "newinstance",
        "ptmxmode=0666",
        "mode=0620",
        "gid=5"
      ]
    },
    {
      "destination": "/dev/shm",
      "type": "tmpfs",
      "source": "shm",
      "options": [
        "nosuid",
        "noexec",
        "nodev",
        "mode=1777",
        "size=65536k"
      ]
    },
    {
      "destination": "/dev/mqueue",
      "type": "mqueue",
      "source": "mqueue",
      "options": [
        "nosuid",
        "noexec",
        "nodev"
      ]
    },
    {
      "destination": "/sys",
      "type": "sysfs",
      "source": "sysfs",
      "options": [
        "nosuid",
        "noexec",
        "nodev",
        "ro"
      ]
    },
    {
      "destination": "/sys/fs/cgroup",
      "type": "cgroup",
      "source": "cgroup",
      "options": [
        "nosuid",
        "noexec",
        "nodev",
        "relatime",
        "ro"
      ]
    }
  ],
  "process": {
    "terminal": false,
    "user": {
      "uid": 0,
      "gid": 0
    },
    "args": [
      "/bin/hello"
    ],
    "env": [
      "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
      "TERM=xterm"
    ],
    "cwd": "/",
    "capabilities": {
      "bounding": [
        "CAP_KILL",
        "CAP_NET_BIND_SERVICE",
        "CAP_AUDIT_WRITE"
      ],
      "effective": [
        "CAP_KILL",
        "CAP_NET_BIND_SERVICE",
        "CAP_AUDIT_WRITE"
      ],
      "inheritable": [
        "CAP_KILL",
        "CAP_NET_BIND_SERVICE",
        "CAP_AUDIT_WRITE"
      ],
      "permitted": [
        "CAP_KILL",
        "CAP_NET_BIND_SERVICE",
        "CAP_AUDIT_WRITE"
      ],
      "ambient": [
        "CAP_KILL",
        "CAP_NET_BIND_SERVICE",
        "CAP_AUDIT_WRITE"
      ]
    },
    "rlimits": [
      {
        "type": "RLIMIT_NOFILE",
        "hard": 1024,
        "soft": 1024
      }
    ],
    "noNewPrivileges": true
  },
  "hostname": "youki",
  "annotations": {},
  "linux": {
    "resources": {
      "devices": [
        {
          "allow": false,
          "type": null,
          "major": null,
          "minor": null,
          "access": "rwm"
        }
      ]
    },
    "namespaces": [
      {
        "type": "pid"
      },
      {
        "type": "network"
      },
      {
        "type": "ipc"
      },
      {
        "type": "uts"
      },
      {
        "type": "mount"
      }
    ],
    "maskedPaths": [
      "/proc/acpi",
      "/proc/asound",
      "/proc/kcore",
      "/proc/keys",
      "/proc/latency_stats",
      "/proc/timer_list",
      "/proc/timer_stats",
      "/proc/sched_debug",
      "/sys/firmware",
      "/proc/scsi"
    ],
    "readonlyPaths": [
      "/proc/bus",
      "/proc/fs",
      "/proc/irq",
      "/proc/sys",
      "/proc/sysrq-trigger"
    ]
  }
}

### Building with musl

In order to build `libcontainer` with musl you must first remove the libseccomp dependency as it will reference shared libraries (`libdbus` and `libseccomp`).

Do this by using the `--no-default-features` flag followed by `-F` and whatever features you intend to build with such as `v2` or `systemd` as defined in Cargo.toml under features section.

```bash
# Add musl to toolchain
rustup target add $(uname -m)-unknown-linux-musl

# Build nigthly stdlib with musl
cargo +nightly build -Zbuild-std --target $(uname -m)-unknown-linux-musl --no-default-features -F v2

# Compile libcontainer without GNU dependencies with musl
cargo +nightly build --target $(uname -m)-unknown-linux-musl --no-default-features -F v2
```