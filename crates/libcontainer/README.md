# libcontainer

### Building with musl

In order to build with musl you must first remove the libseccomp dependency as it will reference shared libraries (`libseccomp`) which cannot be built with musl.

Do this by using adding flags to Cargo. Use the `--no-default-features` flag followed by `-F` and whatever features you intend to build with such as `v2` as defined in Cargo.toml under features section.

Next you will also need the `+nightly` flags when building with `rustup` and `cargo`.

```bash
# Add rustup +nightly musl to toolchain
rustup +nightly target add $(uname -m)-unknown-linux-musl

# Build rustup +nightly stdlib with musl
rustup +nightly toolchain install nightly-$(uname -m)-unknown-linux-musl

# Build musl standard library
cargo +nightly build -Zbuild-std --target $(uname -m)-unknown-linux-musl --no-default-features -F v2

cargo +nightly build --target $(uname -m)-unknown-linux-musl --no-default-features -F v2
```
