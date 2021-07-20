use super::*;
use std::env;

// os and architecture of computer
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Platform {
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
}

/// Gets os and arch of system by default
impl Default for Platform {
    fn default() -> Self {
        Platform {
            os: env::consts::OS.to_string(),
            arch: env::consts::ARCH.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Root {
    // Path to the container's root filesystem
    #[serde(default)]
    pub path: PathBuf,
    // Makes container root file system readonly before process is executed
    #[serde(default)]
    pub readonly: bool,
}

// Default path for container root is "./rootfs" from config.json, with readonly true
impl Default for Root {
    fn default() -> Self {
        Root {
            path: PathBuf::from("rootfs"),
            readonly: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Mount {
    // Path where mount will be placed in container
    #[serde(default)]
    pub destination: PathBuf,
    // Specifies mount type
    #[serde(default, rename = "type")]
    pub typ: String,
    // source path of mount
    #[serde(default)]
    pub source: PathBuf,
    // mount options (https://man7.org/linux/man-pages/man8/mount.8.html)
    #[serde(default)]
    pub options: Vec<String>,
}

// utility function to generate default config for mounts
pub fn get_default_mounts() -> Vec<Mount> {
    let mut default_mounts = Vec::new();
    default_mounts.push(Mount {
        destination: PathBuf::from("/proc"),
        typ: String::from("proc"),
        source: PathBuf::from("proc"),
        options: Vec::new(),
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/dev"),
        typ: String::from("tmpfs"),
        source: PathBuf::from("tmpfs"),
        options: vec![
            "nosuid".to_string(),
            "strictatime".to_string(),
            "mode=755".to_string(),
            "size=65536k".to_string(),
        ],
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/dev/pts"),
        typ: String::from("devpts"),
        source: PathBuf::from("devpts"),
        options: vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "newinstance".to_string(),
            "ptmxmode=0666".to_string(),
            "mode=0620".to_string(),
            "gid=5".to_string(),
        ],
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/dev/shm"),
        typ: String::from("tmpfs"),
        source: PathBuf::from("shm"),
        options: vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
            "mode=1777".to_string(),
            "size=65536k".to_string(),
        ],
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/dev/mqueue"),
        typ: String::from("mqueue"),
        source: PathBuf::from("mqueue"),
        options: vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
        ],
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/sys"),
        typ: String::from("sysfs"),
        source: PathBuf::from("sysfs"),
        options: vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
            "ro".to_string(),
        ],
    });

    default_mounts.push(Mount {
        destination: PathBuf::from("/sys/fs/cgroup"),
        typ: String::from("cgroup"),
        source: PathBuf::from("cgroup"),
        options: vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
            "relatime".to_string(),
            "ro".to_string(),
        ],
    });

    default_mounts
}
