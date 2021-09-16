extern crate libc;

#[allow(non_camel_case_types)]
pub type __s8 = ::std::os::raw::c_schar;
#[allow(non_camel_case_types)]
pub type __u8 = ::std::os::raw::c_uchar;
#[allow(non_camel_case_types)]
pub type __s16 = ::std::os::raw::c_short;
#[allow(non_camel_case_types)]
pub type __u16 = ::std::os::raw::c_ushort;
#[allow(non_camel_case_types)]
pub type __s32 = ::std::os::raw::c_int;
#[allow(non_camel_case_types)]
pub type __u32 = ::std::os::raw::c_uint;
#[allow(non_camel_case_types)]
pub type __s64 = ::std::os::raw::c_longlong;
#[allow(non_camel_case_types)]
pub type __u64 = ::std::os::raw::c_ulonglong;

pub const SCMP_VER_MAJOR: u32 = 2;
pub const SCMP_VER_MINOR: u32 = 5;
pub const SCMP_VER_MICRO: u32 = 1;

pub const __NR_SCMP_ERROR: i32 = -1;
pub const __NR_SCMP_UNDEF: i32 = -2;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum scmp_arch {
    SCMP_ARCH_NATIVE = 0,
    SCMP_ARCH_X86 = 1073741827,
    SCMP_ARCH_X86_64 = 3221225534,
    SCMP_ARCH_X32 = 1073741886,
    SCMP_ARCH_ARM = 1073741864,
    SCMP_ARCH_AARCH64 = 3221225655,
    SCMP_ARCH_MIPS = 8,
    SCMP_ARCH_MIPS64 = 2147483656,
    SCMP_ARCH_MIPS64N32 = 2684354568,
    SCMP_ARCH_MIPSEL = 1073741832,
    SCMP_ARCH_MIPSEL64 = 3221225480,
    SCMP_ARCH_MIPSEL64N32 = 3758096392,
    SCMP_ARCH_PPC = 20,
    SCMP_ARCH_PPC64 = 2147483669,
    SCMP_ARCH_PPC64LE = 3221225493,
    SCMP_ARCH_S390 = 22,
    SCMP_ARCH_S390X = 2147483670,
    SCMP_ARCH_PARISC = 15,
    SCMP_ARCH_PARISC64 = 2147483663,
    SCMP_ARCH_RISCV64 = 3221225715,
}

pub const SCMP_ACT_KILL_PROCESS: u32 = 2147483648;
pub const SCMP_ACT_KILL_THREAD: u32 = 0;
pub const SCMP_ACT_KILL: u32 = 0;
pub const SCMP_ACT_TRAP: u32 = 196608;
pub const SCMP_ACT_NOTIFY: u32 = 2143289344;
pub const SCMP_ACT_LOG: u32 = 2147221504;
pub const SCMP_ACT_ALLOW: u32 = 2147418112;
#[allow(non_snake_case)]
pub fn SCMP_ACT_ERRNO(x: u32) -> u32 {
    0x00050000 | ((x) & 0x0000ffff)
}
#[allow(non_snake_case)]
pub fn SCMP_ACT_TRACE(x: u32) -> u32 {
    0x7ff00000 | ((x) & 0x0000ffff)
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum scmp_filter_attr {
    _SCMP_FLTATR_MIN,
    SCMP_FLTATR_ACT_DEFAULT,
    SCMP_FLTATR_ACT_BADARCH,
    SCMP_FLTATR_CTL_NNP,
    SCMP_FLTATR_CTL_TSYNC,
    SCMP_FLTATR_API_TSKIP,
    SCMP_FLTATR_CTL_LOG,
    SCMP_FLTATR_CTL_SSB,
    SCMP_FLTATR_CTL_OPTIMIZE,
    SCMP_FLTATR_API_SYSRAWRC,
    _SCMP_FLTATR_MAX,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum scmp_compare {
    _SCMP_CMP_MIN = 0,
    SCMP_CMP_NE = 1,
    SCMP_CMP_LT = 2,
    SCMP_CMP_LE = 3,
    SCMP_CMP_EQ = 4,
    SCMP_CMP_GE = 5,
    SCMP_CMP_GT = 6,
    SCMP_CMP_MASKED_EQ = 7,
    _SCMP_CMP_MAX = 8,
}

#[allow(non_camel_case_types)]
pub type scmp_datum_t = u64;

#[allow(non_camel_case_types)]
pub type scmp_filter_ctx = *mut ::std::os::raw::c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct scmp_version {
    pub major: ::std::os::raw::c_uint,
    pub minor: ::std::os::raw::c_uint,
    pub micro: ::std::os::raw::c_uint,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct scmp_arg_cmp {
    pub arg: ::std::os::raw::c_uint,
    pub op: scmp_compare,
    pub datum_a: scmp_datum_t,
    pub datum_b: scmp_datum_t,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct seccomp_data {
    pub nr: ::std::os::raw::c_int,
    pub arch: __u32,
    pub instruction_pointer: __u64,
    pub args: [__u64; 6usize],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct seccomp_notif_sizes {
    pub seccomp_notif: __u16,
    pub seccomp_notif_resp: __u16,
    pub seccomp_data: __u16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct seccomp_notif {
    pub id: __u64,
    pub pid: __u32,
    pub flags: __u32,
    pub data: seccomp_data,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct seccomp_notif_resp {
    pub id: __u64,
    pub val: __s64,
    pub error: __s32,
    pub flags: __u32,
}

#[link(name = "seccomp")]
extern "C" {
    /**
     * Query the library version information
     *
     * This function returns a pointer to a populated scmp_version struct, the
     * caller does not need to free the structure when finished.
     *
     */
    pub fn seccomp_version() -> *const scmp_version;

    /**
     * Query the library's level of API support
     *
     * This function returns an API level value indicating the current supported
     * functionality.  It is important to note that this level of support is
     * determined at runtime and therefore can change based on the running kernel
     * and system configuration (e.g. any previously loaded seccomp filters).  This
     * function can be called multiple times, but it only queries the system the
     * first time it is called, the API level is cached and used in subsequent
     * calls.
     *
     * The current API levels are described below:
     *  0 : reserved
     *  1 : base level
     *  2 : support for the SCMP_FLTATR_CTL_TSYNC filter attribute
     *      uses the seccomp(2) syscall instead of the prctl(2) syscall
     *  3 : support for the SCMP_FLTATR_CTL_LOG filter attribute
     *      support for the SCMP_ACT_LOG action
     *      support for the SCMP_ACT_KILL_PROCESS action
     *  4 : support for the SCMP_FLTATR_CTL_SSB filter attrbute
     *  5 : support for the SCMP_ACT_NOTIFY action and notify APIs
     *  6 : support the simultaneous use of SCMP_FLTATR_CTL_TSYNC and notify APIs
     *
     */
    pub fn seccomp_api_get() -> ::std::os::raw::c_uint;

    /**
     * Set the library's level of API support
     *
     * This function forcibly sets the API level of the library at runtime.  Valid
     * API levels are discussed in the description of the seccomp_api_get()
     * function.  General use of this function is strongly discouraged.
     *
     */
    pub fn seccomp_api_set(level: ::std::os::raw::c_uint) -> ::std::os::raw::c_int;

    /**
     * Initialize the filter state
     * @param def_action the default filter action
     *
     * This function initializes the internal seccomp filter state and should
     * be called before any other functions in this library to ensure the filter
     * state is initialized.  Returns a filter context on success, NULL on failure.
     *
     */
    pub fn seccomp_init(def_action: u32) -> scmp_filter_ctx;

    /**
     * Reset the filter state
     * @param ctx the filter context
     * @param def_action the default filter action
     *
     * This function resets the given seccomp filter state and ensures the
     * filter state is reinitialized.  This function does not reset any seccomp
     * filters already loaded into the kernel.  Returns zero on success, negative
     * values on failure.
     *
     */
    pub fn seccomp_reset(ctx: scmp_filter_ctx, def_action: u32) -> ::std::os::raw::c_int;

    /**
     * Destroys the filter state and releases any resources
     * @param ctx the filter context
     *
     * This functions destroys the given seccomp filter state and releases any
     * resources, including memory, associated with the filter state.  This
     * function does not reset any seccomp filters already loaded into the kernel.
     * The filter context can no longer be used after calling this function.
     *
     */
    pub fn seccomp_release(ctx: scmp_filter_ctx);

    /**
     * Merge two filters
     * @param ctx_dst the destination filter context
     * @param ctx_src the source filter context
     *
     * This function merges two filter contexts into a single filter context and
     * destroys the second filter context.  The two filter contexts must have the
     * same attribute values and not contain any of the same architectures; if they
     * do, the merge operation will fail.  On success, the source filter context
     * will be destroyed and should no longer be used; it is not necessary to
     * call seccomp_release() on the source filter context.  Returns zero on
     * success, negative values on failure.
     *
     */
    pub fn seccomp_merge(
        ctx_dst: scmp_filter_ctx,
        ctx_src: scmp_filter_ctx,
    ) -> ::std::os::raw::c_int;

    /**
     * Resolve the architecture name to a architecture token
     * @param arch_name the architecture name
     *
     * This function resolves the given architecture name to a token suitable for
     * use with libseccomp, returns zero on failure.
     *
     */
    pub fn seccomp_arch_resolve_name(arch_name: *const ::std::os::raw::c_char) -> u32;

    /**
     * Return the native architecture token
     *
     * This function returns the native architecture token value, e.g. SCMP_ARCH_*.
     *
     */
    pub fn seccomp_arch_native() -> u32;

    /**
     * Check to see if an existing architecture is present in the filter
     * @param ctx the filter context
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     *
     * This function tests to see if a given architecture is included in the filter
     * context.  If the architecture token is SCMP_ARCH_NATIVE then the native
     * architecture will be assumed.  Returns zero if the architecture exists in
     * the filter, -EEXIST if it is not present, and other negative values on
     * failure.
     *
     */
    pub fn seccomp_arch_exist(ctx: scmp_filter_ctx, arch_token: u32) -> ::std::os::raw::c_int;

    /**
     * Adds an architecture to the filter
     * @param ctx the filter context
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     *
     * This function adds a new architecture to the given seccomp filter context.
     * Any new rules added after this function successfully returns will be added
     * to this architecture but existing rules will not be added to this
     * architecture.  If the architecture token is SCMP_ARCH_NATIVE then the native
     * architecture will be assumed.  Returns zero on success, -EEXIST if
     * specified architecture is already present, other negative values on failure.
     *
     */
    pub fn seccomp_arch_add(ctx: scmp_filter_ctx, arch_token: u32) -> ::std::os::raw::c_int;

    /**
     * Removes an architecture from the filter
     * @param ctx the filter context
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     *
     * This function removes an architecture from the given seccomp filter context.
     * If the architecture token is SCMP_ARCH_NATIVE then the native architecture
     * will be assumed.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_arch_remove(ctx: scmp_filter_ctx, arch_token: u32) -> ::std::os::raw::c_int;

    /**
     * Loads the filter into the kernel
     * @param ctx the filter context
     *
     * This function loads the given seccomp filter context into the kernel.  If
     * the filter was loaded correctly, the kernel will be enforcing the filter
     * when this function returns.  Returns zero on success, negative values on
     * error.
     *
     */
    pub fn seccomp_load(ctx: scmp_filter_ctx) -> ::std::os::raw::c_int;

    /**
     * Get the value of a filter attribute
     * @param ctx the filter context
     * @param attr the filter attribute name
     * @param value the filter attribute value
     *
     * This function fetches the value of the given attribute name and returns it
     * via @value.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_attr_get(
        ctx: scmp_filter_ctx,
        attr: scmp_filter_attr,
        value: *mut u32,
    ) -> ::std::os::raw::c_int;

    /**
     * Set the value of a filter attribute
     * @param ctx the filter context
     * @param attr the filter attribute name
     * @param value the filter attribute value
     *
     * This function sets the value of the given attribute.  Returns zero on
     * success, negative values on failure.
     *
     */
    pub fn seccomp_attr_set(
        ctx: scmp_filter_ctx,
        attr: scmp_filter_attr,
        value: u32,
    ) -> ::std::os::raw::c_int;

    /**
     * Resolve a syscall number to a name
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     * @param num the syscall number
     *
     * Resolve the given syscall number to the syscall name for the given
     * architecture; it is up to the caller to free the returned string.  Returns
     * the syscall name on success, NULL on failure.
     *
     */
    pub fn seccomp_syscall_resolve_num_arch(
        arch_token: u32,
        num: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_char;

    /**
     * Resolve a syscall name to a number
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     * @param name the syscall name
     *
     * Resolve the given syscall name to the syscall number for the given
     * architecture.  Returns the syscall number on success, including negative
     * pseudo syscall numbers (e.g. __PNR_*); returns __NR_SCMP_ERROR on failure.
     *
     */
    pub fn seccomp_syscall_resolve_name_arch(
        arch_token: u32,
        name: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;

    /**
     * Resolve a syscall name to a number and perform any rewriting necessary
     * @param arch_token the architecture token, e.g. SCMP_ARCH_*
     * @param name the syscall name
     *
     * Resolve the given syscall name to the syscall number for the given
     * architecture and do any necessary syscall rewriting needed by the
     * architecture.  Returns the syscall number on success, including negative
     * pseudo syscall numbers (e.g. __PNR_*); returns __NR_SCMP_ERROR on failure.
     *
     */
    pub fn seccomp_syscall_resolve_name_rewrite(
        arch_token: u32,
        name: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;

    /**
     * Resolve a syscall name to a number
     * @param name the syscall name
     *
     * Resolve the given syscall name to the syscall number.  Returns the syscall
     * number on success, including negative pseudo syscall numbers (e.g. __PNR_*);
     * returns __NR_SCMP_ERROR on failure.
     *
     */
    pub fn seccomp_syscall_resolve_name(
        name: *const ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int;

    /**
     * Set the priority of a given syscall
     * @param ctx the filter context
     * @param syscall the syscall number
     * @param priority priority value, higher value == higher priority
     *
     * This function sets the priority of the given syscall; this value is used
     * when generating the seccomp filter code such that higher priority syscalls
     * will incur less filter code overhead than the lower priority syscalls in the
     * filter.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_syscall_priority(
        ctx: scmp_filter_ctx,
        syscall: ::std::os::raw::c_int,
        priority: u8,
    ) -> ::std::os::raw::c_int;

    /**
     * Add a new rule to the filter
     * @param ctx the filter context
     * @param action the filter action
     * @param syscall the syscall number
     * @param arg_cnt the number of argument filters in the argument filter chain
     * @param ... scmp_arg_cmp structs (use of SCMP_ARG_CMP() recommended)
     *
     * This function adds a series of new argument/value checks to the seccomp
     * filter for the given syscall; multiple argument/value checks can be
     * specified and they will be chained together (AND'd together) in the filter.
     * If the specified rule needs to be adjusted due to architecture specifics it
     * will be adjusted without notification.  Returns zero on success, negative
     * values on failure.
     *
     */
    pub fn seccomp_rule_add(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: ::std::os::raw::c_int,
        arg_cnt: ::std::os::raw::c_uint,
        ...
    ) -> ::std::os::raw::c_int;

    /**
     * Add a new rule to the filter
     * @param ctx the filter context
     * @param action the filter action
     * @param syscall the syscall number
     * @param arg_cnt the number of elements in the arg_array parameter
     * @param arg_array array of scmp_arg_cmp structs
     *
     * This function adds a series of new argument/value checks to the seccomp
     * filter for the given syscall; multiple argument/value checks can be
     * specified and they will be chained together (AND'd together) in the filter.
     * If the specified rule needs to be adjusted due to architecture specifics it
     * will be adjusted without notification.  Returns zero on success, negative
     * values on failure.
     *
     */
    pub fn seccomp_rule_add_array(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: ::std::os::raw::c_int,
        arg_cnt: ::std::os::raw::c_uint,
        arg_array: *const scmp_arg_cmp,
    ) -> ::std::os::raw::c_int;

    /**
     * Add a new rule to the filter
     * @param ctx the filter context
     * @param action the filter action
     * @param syscall the syscall number
     * @param arg_cnt the number of argument filters in the argument filter chain
     * @param ... scmp_arg_cmp structs (use of SCMP_ARG_CMP() recommended)
     *
     * This function adds a series of new argument/value checks to the seccomp
     * filter for the given syscall; multiple argument/value checks can be
     * specified and they will be chained together (AND'd together) in the filter.
     * If the specified rule can not be represented on the architecture the
     * function will fail.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_rule_add_exact(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: ::std::os::raw::c_int,
        arg_cnt: ::std::os::raw::c_uint,
        ...
    ) -> ::std::os::raw::c_int;

    /**
     * Add a new rule to the filter
     * @param ctx the filter context
     * @param action the filter action
     * @param syscall the syscall number
     * @param arg_cnt  the number of elements in the arg_array parameter
     * @param arg_array array of scmp_arg_cmp structs
     *
     * This function adds a series of new argument/value checks to the seccomp
     * filter for the given syscall; multiple argument/value checks can be
     * specified and they will be chained together (AND'd together) in the filter.
     * If the specified rule can not be represented on the architecture the
     * function will fail.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_rule_add_exact_array(
        ctx: scmp_filter_ctx,
        action: u32,
        syscall: ::std::os::raw::c_int,
        arg_cnt: ::std::os::raw::c_uint,
        arg_array: *const scmp_arg_cmp,
    ) -> ::std::os::raw::c_int;

    /**
     * Allocate a pair of notification request/response structures
     * @param req the request location
     * @param resp the response location
     *
     * This function allocates a pair of request/response structure by computing
     * the correct sized based on the currently running kernel. It returns zero on
     * success, and negative values on failure.
     *
     */
    pub fn seccomp_notify_alloc(
        req: *mut *mut seccomp_notif,
        resp: *mut *mut seccomp_notif_resp,
    ) -> ::std::os::raw::c_int;

    /**
     * Free a pair of notification request/response structures.
     * @param req the request location
     * @param resp the response location
     */
    pub fn seccomp_notify_free(req: *mut seccomp_notif, resp: *mut seccomp_notif_resp);

    /**
     * Receive a notification from a seccomp notification fd
     * @param fd the notification fd
     * @param req the request buffer to save into
     *
     * Blocks waiting for a notification on this fd. This function is thread safe
     * (synchronization is performed in the kernel). Returns zero on success,
     * negative values on error.
     *
     */
    pub fn seccomp_notify_receive(
        fd: ::std::os::raw::c_int,
        req: *mut seccomp_notif,
    ) -> ::std::os::raw::c_int;

    /**
     * Send a notification response to a seccomp notification fd
     * @param fd the notification fd
     * @param resp the response buffer to use
     *
     * Sends a notification response on this fd. This function is thread safe
     * (synchronization is performed in the kernel). Returns zero on success,
     * negative values on error.
     *
     */
    pub fn seccomp_notify_respond(
        fd: ::std::os::raw::c_int,
        resp: *mut seccomp_notif_resp,
    ) -> ::std::os::raw::c_int;

    /**
     * Check if a notification id is still valid
     * @param fd the notification fd
     * @param id the id to test
     *
     * Checks to see if a notification id is still valid. Returns 0 on success, and
     * negative values on failure.
     *
     */
    pub fn seccomp_notify_id_valid(fd: ::std::os::raw::c_int, id: u64) -> ::std::os::raw::c_int;

    /**
     * Return the notification fd from a filter that has already been loaded
     * @param ctx the filter context
     *
     * This returns the listener fd that was generated when the seccomp policy was
     * loaded. This is only valid after seccomp_load() with a filter that makes
     * use of SCMP_ACT_NOTIFY.
     *
     */
    pub fn seccomp_notify_fd(ctx: scmp_filter_ctx) -> ::std::os::raw::c_int;

    /**
     * Generate seccomp Pseudo Filter Code (PFC) and export it to a file
     * @param ctx the filter context
     * @param fd the destination fd
     *
     * This function generates seccomp Pseudo Filter Code (PFC) and writes it to
     * the given fd.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_export_pfc(
        ctx: scmp_filter_ctx,
        fd: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;

    /**
     * Generate seccomp Berkeley Packet Filter (BPF) code and export it to a file
     * @param ctx the filter context
     * @param fd the destination fd
     *
     * This function generates seccomp Berkeley Packer Filter (BPF) code and writes
     * it to the given fd.  Returns zero on success, negative values on failure.
     *
     */
    pub fn seccomp_export_bpf(
        ctx: scmp_filter_ctx,
        fd: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // Note: we should probably run this in a different process, since it
        // loads a seccomp profile. However, since this is the only test in the
        // repo at the moment, this should be OK for now.
        unsafe {
            let ctx = seccomp_init(SCMP_ACT_ALLOW);
            let cmp = scmp_arg_cmp {
                arg: 0,
                op: scmp_compare::SCMP_CMP_EQ,
                datum_a: 1000,
                datum_b: 0,
            };

            let c_syscall_name = std::ffi::CString::new("getcwd").unwrap();
            let syscall_number = seccomp_syscall_resolve_name(c_syscall_name.as_ptr());

            assert!(seccomp_rule_add(ctx, SCMP_ACT_ERRNO(42), syscall_number, 1, cmp) == 0);
            assert!(seccomp_load(ctx) == 0);
        }
    }
}
