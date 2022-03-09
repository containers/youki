// Here we duplicate the signatures of external functions and apply
// the mockall::automock macro to generate mock modules for use
// in tests, allowing us to exercise code paths without eg making syscalls

#[cfg_attr(test, automock())]
pub mod libc {
    pub fn setrlimit(
        _resource: libc::__rlimit_resource_t,
        _rlim: *const libc::rlimit,
    ) -> libc::c_int {
        unimplemented!();
    }
}

#[cfg_attr(test, automock())]
pub mod libbpf_sys {
    pub fn bpf_load_program(
        _type_: libbpf_sys::bpf_prog_type,
        _insns: *const libbpf_sys::bpf_insn,
        _insns_cnt: libbpf_sys::size_t,
        _license: *const ::std::os::raw::c_char,
        _kern_version: libbpf_sys::__u32,
        _log_buf: *mut ::std::os::raw::c_char,
        _log_buf_sz: libbpf_sys::size_t,
    ) -> ::std::os::raw::c_int {
        unimplemented!();
    }

    pub fn bpf_prog_query(
        _target_fd: ::std::os::raw::c_int,
        _type_: libbpf_sys::bpf_attach_type,
        _query_flags: libbpf_sys::__u32,
        _attach_flags: *mut libbpf_sys::__u32,
        _prog_ids: *mut libbpf_sys::__u32,
        _prog_cnt: *mut libbpf_sys::__u32,
    ) -> ::std::os::raw::c_int {
        unimplemented!();
    }

    pub fn bpf_prog_get_fd_by_id(_id: libbpf_sys::__u32) -> ::std::os::raw::c_int {
        unimplemented!();
    }

    pub fn bpf_prog_detach2(
        _prog_fd: ::std::os::raw::c_int,
        _attachable_fd: ::std::os::raw::c_int,
        _type_: libbpf_sys::bpf_attach_type,
    ) -> ::std::os::raw::c_int {
        unimplemented!();
    }

    pub fn bpf_prog_attach(
        _prog_fd: ::std::os::raw::c_int,
        _attachable_fd: ::std::os::raw::c_int,
        _type_: libbpf_sys::bpf_attach_type,
        _flags: ::std::os::raw::c_uint,
    ) -> ::std::os::raw::c_int {
        unimplemented!();
    }
}
