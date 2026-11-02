// `/proc` exists only on Linux, where its one use, `process_memory_kib`, is built.
#[cfg(target_os = "linux")]
macro_rules! linux_proc_self_status_read_to_string {
    () => {{
        ::std::fs::read_to_string("/proc/self/status")
    }};
}
