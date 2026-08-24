macro_rules! linux_proc_self_status_read_to_string {
    () => {{
        ::std::fs::read_to_string("/proc/self/status")
    }};
}
