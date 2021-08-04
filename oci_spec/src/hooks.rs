use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Hooks specifies a command that is run in the container at a particular event in the lifecycle
/// (setup and teardown) of a container.
pub struct Hooks {
    #[deprecated(
        note = "Prestart hooks were deprecated in favor of `createRuntime`, `createContainer` and `startContainer` hooks"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The `prestart` hooks MUST be called after the `start` operation is called but before the
    /// user-specified program command is executed.
    ///
    /// On Linux, for example, they are called after the container namespaces are created, so they
    /// provide an opportunity to customize the container (e.g. the network namespace could be
    /// specified in this hook).
    ///
    /// The `prestart` hooks' path MUST resolve in the runtime namespace.
    /// The `prestart` hooks MUST be executed in the runtime namespace.
    pub prestart: Option<Vec<Hook>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CreateRuntime is a list of hooks to be run after the container has been created but before
    /// `pivot_root` or any equivalent operation has been called. It is called in the Runtime
    /// Namespace.
    pub create_runtime: Option<Vec<Hook>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CreateContainer is a list of hooks to be run after the container has been created but
    /// before `pivot_root` or any equivalent operation has been called. It is called in the
    /// Container Namespace.
    pub create_container: Option<Vec<Hook>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// StartContainer is a list of hooks to be run after the start operation is called but before
    /// the container process is started. It is called in the Container Namespace.
    pub start_container: Option<Vec<Hook>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Poststart is a list of hooks to be run after the container process is started. It is called
    /// in the Runtime Namespace.
    pub poststart: Option<Vec<Hook>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Poststop is a list of hooks to be run after the container process exits. It is called in
    /// the Runtime Namespace.
    pub poststop: Option<Vec<Hook>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// Hook specifies a command that is run at a particular event in the lifecycle of a container.
pub struct Hook {
    /// Path to the binary to be executed. Following similar semantics to [IEEE Std 1003.1-2008
    /// `execv`'s path](https://pubs.opengroup.org/onlinepubs/9699919799/functions/exec.html). This
    /// specification extends the IEEE standard in that path MUST be absolute.
    pub path: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arguments used for the binary, including the binary name itself. Following the same
    /// semantics as [IEEE Std 1003.1-2008 `execv`'s
    /// argv](https://pubs.opengroup.org/onlinepubs/9699919799/functions/exec.html).
    pub args: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Additional `key=value` environment variables. Following the same semantics as [IEEE Std
    /// 1003.1-2008's `environ`](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html#tag_08_01).
    pub env: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Timeout is the number of seconds before aborting the hook. If set, timeout MUST be greater
    /// than zero.
    pub timeout: Option<i64>,
}
