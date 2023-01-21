# libcgroups

Youki crate for creating and managing cgroups.

By default, youki will determine which cgroup hierarchy to use based on the state of the system at runtime.

The cgroup implementation uses a public trait called `CgroupManager` which can be an implementation of:

 - Cgroup v1 hierarchy
 - Cgroup v2 hierarchy

```rust
pub trait CgroupManager {
    fn add_task(&self, pid: Pid) -> Result<()>;
    fn apply(&self, controller_opt: &ControllerOpt) -> Result<()>;
    fn remove(&self) -> Result<()>;
    fn freeze(&self, state: FreezerState) -> Result<()>;
    fn stats(&self) -> Result<Stats>;
    fn get_all_pids(&self) -> Result<Vec<Pid>>;
}
```

The determination is made by `get_cgroup_setup()` function.

```rust 
/// Determines the cgroup setup of the system. Systems typically have one of
/// three setups:
/// - Unified: Pure cgroup v2 system.
/// - Legacy: Pure cgroup v1 system.
/// - Hybrid: Hybrid is basically a cgroup v1 system, except for
///   an additional unified hierarchy which doesn't have any
///   controllers attached. Resource control can purely be achieved
///   through the cgroup v1 hierarchy, not through the cgroup v2 hierarchy.
pub fn get_cgroup_setup() -> Result<CgroupSetup>
```
### Running Examples

There are example files in the `libcgroup/examples/*.rs` pattern.

The nightly cargo build is required to run the examples.

Run the examples below as follows:

```bash 
rustup install nightly # Install nightly cargo
cargo +nightly run --example examples/bpf    # Run BPF example
cargo +nightly run --example examples/create # Run create example 
```
