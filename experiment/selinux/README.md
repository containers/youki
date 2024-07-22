This is an experimental project in order to create selinux library in Rust.  
Ref: https://github.com/containers/youki/issues/2718.  
Reimplementation of [opencontainers/selinux](https://github.com/opencontainers/selinux) in Rust.  
Also selinux depends on xattr, but nix doesn't cover xattr function. 
Therefore, this PR will implement xattr in Rust.  
Referenced the implementation of xattr in [unix](golang.org/x/sys/unix) repo.  

Please import and use this project.
