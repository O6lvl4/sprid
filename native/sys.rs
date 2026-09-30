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

/// Whether this is Linux: its shortcuts are Ctrl+Shift's, a Mac's Cmd's.
pub fn is_linux() -> bool { cfg!(target_os = "linux") }

/// The directory this process runs in.
pub fn cwd() -> String {
    std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The bytes of the file at `path`, mapped rather than read (see
/// `mapped.rs`); empty when it can't be read. For fonts loaded the moment a
/// character first needs one, from code that isn't an effect.
pub fn read_file(path: &str) -> crate::AlmideRcCow<Vec<u8>> {
    crate::AlmideRcCow::new(crate::mapped::map_file(path).unwrap_or_default())
}

/// Give this process — and so the shell and everything it runs, pbcopy
/// included — a UTF-8 locale when it has none. Started from Finder, a program
/// gets no LANG: the shell then runs in the C locale, counts `❯` as 3 columns
/// and garbles every non-ASCII name. The locale is the system's (`ja_JP`,
/// say) in UTF-8 when the system has that one, else `en_US.UTF-8`; LANG, LC_ALL
/// or LC_CTYPE already set are left alone.
pub fn ensure_utf8_locale() {
    let set = |k: &str| std::env::var(k).map_or(false, |v| !v.is_empty());
    if set("LC_ALL") || set("LC_CTYPE") || set("LANG") {
        return;
    }
    let valid = |name: &str| {
        let Ok(c) = std::ffi::CString::new(name) else { return false };
        unsafe {
            let ok = !libc::setlocale(libc::LC_CTYPE, c.as_ptr()).is_null();
            libc::setlocale(libc::LC_CTYPE, c"C".as_ptr());
            ok
        }
    };
    // C.UTF-8 is there on any Linux; en_US.UTF-8 only when generated.
    let chosen = system_locale()
        .map(|id| format!("{id}.UTF-8"))
        .into_iter()
        .chain(["en_US.UTF-8".to_string(), "C.UTF-8".to_string()])
        .find(|name| valid(name))
        .unwrap_or_else(|| "en_US.UTF-8".to_string());
    std::env::set_var("LANG", chosen);
}

/// The system's locale identifier, `ja_JP` say, without `@` modifiers.
#[cfg(target_os = "macos")]
fn system_locale() -> Option<String> {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFLocaleCopyCurrent() -> *const c_void;
        fn CFLocaleGetIdentifier(locale: *const c_void) -> *const c_void;
        fn CFStringGetCString(s: *const c_void, buf: *mut i8, size: isize, encoding: u32) -> u8;
        fn CFRelease(cf: *const c_void);
    }
    const UTF8: u32 = 0x0800_0100;
    unsafe {
        let locale = CFLocaleCopyCurrent();
        if locale.is_null() { return None; }
        let mut buf = [0i8; 128];
        let ok = CFStringGetCString(CFLocaleGetIdentifier(locale), buf.as_mut_ptr(), buf.len() as isize, UTF8) != 0;
        CFRelease(locale);
        if !ok { return None; }
        let id = std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        let id = id.split('@').next().unwrap_or("").to_string();
        if id.is_empty() { None } else { Some(id) }
    }
}

#[cfg(not(target_os = "macos"))]
fn system_locale() -> Option<String> { None }

/// The commands that write the clipboard from their stdin and print it:
/// pbcopy / pbpaste on macOS; on Linux wl-copy / wl-paste under Wayland,
/// else xclip or xsel under X11, whichever is installed.
fn clipboard_tools() -> Option<(&'static [&'static str], &'static [&'static str])> {
    if cfg!(target_os = "macos") {
        return Some((&["pbcopy"], &["pbpaste"]));
    }
    let on_path = |cmd: &str| {
        std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
    };
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland && on_path("wl-copy") && on_path("wl-paste") {
        Some((&["wl-copy"], &["wl-paste", "--no-newline"]))
    } else if on_path("xclip") {
        Some((&["xclip", "-selection", "clipboard"], &["xclip", "-selection", "clipboard", "-o"]))
    } else if on_path("xsel") {
        Some((&["xsel", "--clipboard", "--input"], &["xsel", "--clipboard", "--output"]))
    } else {
        None
    }
}

fn command(argv: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new(argv[0]);
    cmd.args(&argv[1..]);
    cmd
}

/// Put `text` on the clipboard; `false` when there is no way to.
pub fn clipboard_set(text: &str) -> bool {
    use std::io::Write;
    use std::process::Stdio;
    let Some((copy, _)) = clipboard_tools() else { return false };
    // The X11 and Wayland tools stay behind to serve the selection: their
    // output goes nowhere, or reading it would wait for them to exit.
    let Ok(mut child) = command(copy).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    let wrote = child.stdin.take().map_or(false, |mut i| i.write_all(text.as_bytes()).is_ok());
    child.wait().map_or(false, |s| s.success()) && wrote
}

/// The clipboard's text; empty when there is none, or no way to read it.
pub fn clipboard_get() -> String {
    let Some((_, paste)) = clipboard_tools() else { return String::new() };
    command(paste)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Start another sprid — a window of its own — in `cwd` (the inherited
/// directory when empty); `false` when it can't be started. It is reaped
/// with the shells (`pty::reap` waits for any child), and outlives this one.
pub fn open_window(cwd: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let mut cmd = std::process::Command::new(exe);
    if !cwd.is_empty() {
        cmd.current_dir(cwd);
    }
    use std::process::Stdio;
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}
