//! Process facts for sprid's benchmarks. Backs `src/sys.almd`.

/// Physical memory this process uses now, in bytes: the phys_footprint
/// macOS reports in Activity Monitor (resident memory elsewhere).
pub fn footprint() -> i64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut info: libc::rusage_info_v2 = std::mem::zeroed();
        let r = libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V2,
            &mut info as *mut _ as *mut libc::rusage_info_t,
        );
        if r == 0 { return info.ri_phys_footprint as i64; }
        -1
    }
    #[cfg(not(target_os = "macos"))]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
        let pages: i64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(-1);
        if pages < 0 { -1 } else { pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) as i64 } }
    }
}

/// Monotonic milliseconds.
pub fn now_ms() -> i64 {
    use std::time::Instant;
    thread_local! { static START: Instant = Instant::now(); }
    START.with(|s| s.elapsed().as_millis() as i64)
}

/// The user's login shell from the password database: what a program
/// started from Finder has instead of $SHELL.
pub fn login_shell() -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_shell.is_null() { return String::new(); }
        std::ffi::CStr::from_ptr((*pw).pw_shell).to_string_lossy().into_owned()
    }
}

/// The directory this process runs in.
pub fn cwd() -> String {
    std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}
