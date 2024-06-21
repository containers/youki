This is an experimental project in order to create selinux library in Rust.  
Ref: https://github.com/containers/youki/issues/2718.  
Reimplementation of (selinux)[https://github.com/opencontainers/selinux] in Rust.  
Also selinux depends on xattr, but nix doesn't cover xattr function. 
Therefore, this PR will implement xattr in Rust.  

Please import and use this project.
