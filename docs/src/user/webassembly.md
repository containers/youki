# Webassembly

If you want to run a webassembly module with youki, your config.json has to include either **runc.oci.handler** or **module.wasm.image/variant=compat"**. 

It also needs to specifiy a valid .wasm (webassembly binary) or .wat (webassembly test) module as entrypoint for the container. If a wat module is specified it will be compiled to a wasm module by youki before it is executed. The module also needs to be available in the root filesystem of the container obviously.


```json
"ociVersion": "1.0.2-dev",
"annotations": {
	"run.oci.handler": "wasm"
},
"process": {
    "args": [
		"hello.wasm",
		"hello",
        "world"
		],
...
```
Lastly you need to ensure that youki was compiled with the wasm-wasmer feature in order for youki to be able to execute the module. Otherwise youki will not know how to execute the wasm module.

A simple wasm module can be created by running 

```console
rustup target add wasm32-wasi
cargo new wasm-module --bin
```

```rust
fn main() {
    println!("Printing args");
    for arg in std::env::args().skip(1) {
        println!("{}", arg);
    }

    println!("Printing envs");
    for envs in std::env::vars() {
        println!("{:?}", envs);
    }  
}
```
Then compile the program to WASI.
```console
cargo build --target wasm32-wasi
```