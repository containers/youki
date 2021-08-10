use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

mod hooks;
mod linux;
mod miscellaneous;
mod process;
mod solaris;
mod test;
mod vm;
mod windows;

// re-export for ease of use
pub use hooks::*;
pub use linux::*;
pub use miscellaneous::*;
pub use process::*;
pub use solaris::*;
pub use vm::*;
pub use windows::*;

/// Base configuration for the container.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Spec {
    #[serde(default, rename = "ociVersion")]
    ///  MUST be in SemVer v2.0.0 format and specifies the version of the Open Container Initiative
    ///  Runtime Specification with which the bundle complies. The Open Container Initiative
    ///  Runtime Specification follows semantic versioning and retains forward and backward
    ///  compatibility within major versions. For example, if a configuration is compliant with
    ///  version 1.1 of this specification, it is compatible with all runtimes that support any 1.1
    ///  or later release of this specification, but is not compatible with a runtime that supports
    ///  1.0 and not 1.1.
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies the container's root filesystem. On Windows, for Windows Server Containers, this
    /// field is REQUIRED. For Hyper-V Containers, this field MUST NOT be set.
    ///
    /// On all other platforms, this field is REQUIRED.
    pub root: Option<Root>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies additional mounts beyond `root`. The runtime MUST mount entries in the listed
    /// order.
    ///
    /// For Linux, the parameters are as documented in
    /// [`mount(2)`](http://man7.org/linux/man-pages/man2/mount.2.html) system call man page. For
    /// Solaris, the mount entry corresponds to the 'fs' resource in the
    /// [`zonecfg(1M)`](http://docs.oracle.com/cd/E86824_01/html/E54764/zonecfg-1m.html) man page.
    pub mounts: Option<Vec<Mount>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies the container process. This property is REQUIRED when
    /// [`start`](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md#start) is
    /// called.
    pub process: Option<Process>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies the container's hostname as seen by processes running inside the container. On
    /// Linux, for example, this will change the hostname in the container [UTS
    /// namespace](http://man7.org/linux/man-pages/man7/namespaces.7.html). Depending on your
    /// [namespace
    /// configuration](https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#namespaces),
    /// the container UTS namespace may be the runtime UTS namespace.
    pub hostname: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Hooks allow users to specify programs to run before or after various lifecycle events.
    /// Hooks MUST be called in the listed order. The state of the container MUST be passed to
    /// hooks over stdin so that they may do work appropriate to the current state of the
    /// container.
    pub hooks: Option<Hooks>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Annotations contains arbitrary metadata for the container. This information MAY be
    /// structured or unstructured. Annotations MUST be a key-value map. If there are no
    /// annotations then this property MAY either be absent or an empty map.
    ///
    /// Keys MUST be strings. Keys MUST NOT be an empty string. Keys SHOULD be named using a
    /// reverse domain notation - e.g. com.example.myKey. Keys using the org.opencontainers
    /// namespace are reserved and MUST NOT be used by subsequent specifications. Runtimes MUST
    /// handle unknown annotation keys like any other unknown property.
    ///
    /// Values MUST be strings. Values MAY be an empty string.
    pub annotations: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Linux is platform-specific configuration for Linux based containers.
    pub linux: Option<Linux>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Solaris is platform-specific configuration for Solaris based containers.
    pub solaris: Option<Solaris>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Windows is platform-specific configuration for Windows based containers.
    pub windows: Option<Windows>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// VM specifies configuration for Virtual Machine based containers.
    pub vm: Option<VM>,
}

// This gives a basic boilerplate for Spec that can be used calling Default::default().
// The values given are similar to the defaults seen in docker and runc, it creates a containerized shell!
// (see respective types default impl for more info)
impl Default for Spec {
    fn default() -> Self {
        Spec {
            // Defaults to most current oci version
            version: String::from("1.0.2-dev"),
            process: Some(Default::default()),
            root: Some(Default::default()),
            // Defaults hostname as youki
            hostname: "youki".to_string().into(),
            mounts: get_default_mounts().into(),
            // Defaults to empty metadata
            annotations: Some(Default::default()),
            linux: Some(Default::default()),
            hooks: None,
            solaris: None,
            windows: None,
            vm: None,
        }
    }
}

impl Spec {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path)
            .with_context(|| format!("load spec: failed to open {:?}", path))?;
        let spec: Spec = serde_json::from_reader(&file)?;
        Ok(spec)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = fs::File::create(path)
            .with_context(|| format!("save spec: failed to create/open {:?}", path))?;
        serde_json::to_writer(&file, self)
            .with_context(|| format!("failed to save spec to {:?}", path))?;

        Ok(())
    }

    pub fn canonicalize_rootfs<P: AsRef<Path>>(&mut self, bundle: P) -> Result<()> {
        let root = self.root.as_mut().context("no root path provided")?;
        root.path = Self::canonicalize_path(bundle, &root.path)?;
        Ok(())
    }

    fn canonicalize_path<B, P>(bundle: B, path: P) -> Result<PathBuf>
    where
        B: AsRef<Path>,
        P: AsRef<Path>,
    {
        Ok(if path.as_ref().is_absolute() {
            fs::canonicalize(path.as_ref())
                .with_context(|| format!("failed to canonicalize {}", path.as_ref().display()))?
        } else {
            let canonical_bundle_path = fs::canonicalize(&bundle).context(format!(
                "failed to canonicalize bundle: {}",
                bundle.as_ref().display()
            ))?;

            fs::canonicalize(canonical_bundle_path.join(path.as_ref())).context(format!(
                "failed to canonicalize rootfs: {}",
                path.as_ref().display()
            ))?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_rootfs() -> Result<()> {
        let rootfs_name = "rootfs";
        let bundle = tempfile::tempdir().with_context(|| "Failed to create tmp test bundle dir")?;
        let rootfs_absolute_path = bundle.path().join(rootfs_name);
        assert!(
            rootfs_absolute_path.is_absolute(),
            "rootfs path is not absolute path"
        );
        fs::create_dir_all(&rootfs_absolute_path)
            .with_context(|| "Failed to create the testing rootfs")?;
        {
            // Test the case with absolute path
            let mut spec = Spec {
                root: Root {
                    path: rootfs_absolute_path.clone(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            };
            spec.canonicalize_rootfs(bundle.path())
                .with_context(|| "Failed to canonicalize rootfs")?;
            assert_eq!(
                rootfs_absolute_path,
                spec.root.context("no root in spec")?.path
            );
        }

        {
            // Test the case with relative path
            let mut spec = Spec {
                root: Root {
                    path: PathBuf::from(rootfs_name),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            };
            spec.canonicalize_rootfs(bundle.path())
                .with_context(|| "Failed to canonicalize rootfs")?;
            assert_eq!(
                rootfs_absolute_path,
                spec.root.context("no root in spec")?.path
            );
        }

        Ok(())
    }

    #[test]
    fn test_load_save() -> Result<()> {
        let spec = Spec {
            ..Default::default()
        };
        let test_dir = tempfile::tempdir().with_context(|| "Failed to create tmp test dir")?;
        let spec_path = test_dir.into_path().join("config.json");

        // Test first save the default config, and then load the saved config.
        // The before and after should be the same.
        spec.save(&spec_path)
            .with_context(|| "Failed to save spec")?;
        let loaded_spec =
            Spec::load(&spec_path).with_context(|| "Failed to load the saved spec.")?;
        assert_eq!(
            spec, loaded_spec,
            "The saved spec is not the same as the loaded spec"
        );

        Ok(())
    }
}
