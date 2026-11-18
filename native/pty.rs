//! PTY host for sprid: the only part of the terminal that talks to the OS.
//!
//! Backs `src/pty.almd`. Everything above byte I/O on the master fd — parsing,
//! screen state, scrollback — is Almide.

use crate::AlmideRcCow;
use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    /// Master fds whose child side has closed: a read hit the end.
    static ENDED: RefCell<HashSet<i32>> = RefCell::new(HashSet::new());
    static SCRATCH: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 65536]);
    /// The pid of the program each master fd was started with (the shell).
    static STARTED: RefCell<std::collections::HashMap<i32, i32>> = RefCell::new(std::collections::HashMap::new());
    /// Input the child hasn't taken yet, per master fd: written as it reads.
    static PENDING: RefCell<std::collections::HashMap<i32, Vec<u8>>> = RefCell::new(std::collections::HashMap::new());
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// A tab's program is read by a thread of its own, and the loop takes what
/// it collected in one piece. macOS hands a PTY over 1 KB at a time, each KB
/// a handoff between the program and its reader: done on the loop's thread,
/// the parsing sat between one KB and the next, and `cat` of a big file ran
/// at the pace of the two taking turns. The thread does nothing but read —
/// trying again for a moment when the KB is not there yet, as a streaming
/// program refills it within microseconds, then sleeping in poll — while the
/// loop parses what came.
struct Chan {
    queued: Mutex<Queued>,
    /// Signalled when output comes, the end comes, or the queue is taken.
    changed: Condvar,
    /// Readable while the queue holds output or has ended: what the loop
    /// sleeps on in place of the master (`wake_fd`).
    wake_r: i32,
    wake_w: i32,
    /// Written once to end the thread.
    stop_r: i32,
    stop_w: i32,
    stop: AtomicBool,
}

struct Queued {
    data: Vec<u8>,
    eof: bool,
    /// A byte waits in the wake pipe.
    signalled: bool,
}

/// The most output a tab keeps unread before its program is made to wait.
const QUEUE_LIMIT: usize = 4 << 20;
/// The most one `read` hands over, so the loop looks at the clock between
/// pieces.
const READ_MAX: usize = 256 << 10;
/// How many empty reads the thread tries before it sleeps: each a few
/// hundred ns, the wake-up from poll several µs.
const SPINS: u32 = 200;

fn chans() -> &'static Mutex<std::collections::HashMap<i32, Arc<Chan>>> {
    static CHANS: OnceLock<Mutex<std::collections::HashMap<i32, Arc<Chan>>>> = OnceLock::new();
    CHANS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn chan(fd: i64) -> Option<Arc<Chan>> { chans().lock().unwrap().get(&(fd as i32)).cloned() }

/// A pipe neither end of which a child inherits, both non-blocking.
fn pipe_cloexec() -> Option<(i32, i32)> {
    let mut p = [0i32; 2];
    if unsafe { libc::pipe(p.as_mut_ptr()) } != 0 { return None; }
    for f in p {
        unsafe {
            libc::fcntl(f, libc::F_SETFD, libc::FD_CLOEXEC);
            let fl = libc::fcntl(f, libc::F_GETFL);
            libc::fcntl(f, libc::F_SETFL, fl | libc::O_NONBLOCK);
        }
    }
    Some((p[0], p[1]))
}

/// Start reading `master` on a thread of its own.
fn start_reader(master: i32) {
    let Some((wake_r, wake_w)) = pipe_cloexec() else { return };
    let Some((stop_r, stop_w)) = pipe_cloexec() else {
        unsafe { libc::close(wake_r); libc::close(wake_w); }
        return;
    };
    let ch = Arc::new(Chan {
        queued: Mutex::new(Queued { data: Vec::new(), eof: false, signalled: false }),
        changed: Condvar::new(),
        wake_r, wake_w, stop_r, stop_w,
        stop: AtomicBool::new(false),
    });
    chans().lock().unwrap().insert(master, ch.clone());
    let started = std::thread::Builder::new().name(format!("pty-{master}")).spawn(move || reader(master, ch));
    if started.is_err() {
        if let Some(ch) = chans().lock().unwrap().remove(&master) {
            unsafe { libc::close(ch.wake_r); libc::close(ch.wake_w); libc::close(ch.stop_r); libc::close(ch.stop_w); }
        }
    }
}

/// The thread: `fd`'s output into the queue until the end or `stop`; closes
/// `fd` and `stop_r` on the way out.
fn reader(fd: i32, ch: Arc<Chan>) {
    let mut buf = vec![0u8; 65536];
    let mut spins = 0u32;
    while !ch.stop.load(Ordering::Relaxed) {
        {
            let mut q = ch.queued.lock().unwrap();
            while q.data.len() >= QUEUE_LIMIT && !ch.stop.load(Ordering::Relaxed) {
                q = ch.changed.wait(q).unwrap();
            }
        }
        let r = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if r > 0 {
            let mut q = ch.queued.lock().unwrap();
            q.data.extend_from_slice(&buf[..r as usize]);
            signal(&ch, &mut q);
            ch.changed.notify_all();
            spins = 0;
            continue;
        }
        let e = if r == 0 { 0 } else { errno() };
        if e == libc::EINTR { continue; }
        if e != libc::EAGAIN {
            // 0, or EIO once the child side is closed: the end. Wait to be
            // stopped, not to be read again.
            {
                let mut q = ch.queued.lock().unwrap();
                q.eof = true;
                signal(&ch, &mut q);
                ch.changed.notify_all();
            }
            let mut p = libc::pollfd { fd: ch.stop_r, events: libc::POLLIN, revents: 0 };
            unsafe { libc::poll(&mut p, 1, -1) };
            break;
        }
        if spins < SPINS {
            std::hint::spin_loop();
            spins += 1;
            continue;
        }
        spins = 0;
        let mut p = [
            libc::pollfd { fd, events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: ch.stop_r, events: libc::POLLIN, revents: 0 },
        ];
        unsafe { libc::poll(p.as_mut_ptr(), 2, -1) };
    }
    unsafe { libc::close(fd); libc::close(ch.stop_r); }
}

/// A byte into the wake pipe, unless one waits there already. Under the
/// queue's lock, as is `close`, so the pipe is never written after it went.
fn signal(ch: &Chan, q: &mut Queued) {
    if !q.signalled && !ch.stop.load(Ordering::Relaxed) {
        let b = 1u8;
        unsafe { libc::write(ch.wake_w, &b as *const u8 as *const _, 1) };
        q.signalled = true;
    }
}

/// The fd the loop sleeps on for output of `fd`'s program.
pub fn wake_fd(fd: i64) -> i64 { chan(fd).map(|c| c.wake_r as i64).unwrap_or(fd) }

/// Start `program` (run through `/bin/sh -c`) on a new PTY of `cols` x `rows`,
/// in directory `cwd` when it is not empty. Returns the master fd, or -1.
///
/// Everything the child needs is made before the fork: between fork and exec
/// a multithreaded process may only make async-signal-safe calls, and an
/// allocation or the environment's lock could deadlock the new tab.
pub fn spawn(program: &str, cwd: &str, cols: i64, rows: i64) -> i64 {
    use std::ffi::CString;
    let ws = libc::winsize { ws_row: rows as u16, ws_col: cols as u16, ws_xpixel: 0, ws_ypixel: 0 };
    let mut master: libc::c_int = -1;
    let sh = CString::new("/bin/sh").unwrap();
    let dash_c = CString::new("-c").unwrap();
    let Ok(cmd) = CString::new(program) else { return -1 };
    let dir = CString::new(cwd).ok().filter(|d| !d.as_bytes().is_empty());
    // What the child sees of its terminal: xterm's sequences, direct colour.
    let set = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "sprid")];
    let mut env: Vec<CString> = std::env::vars_os()
        .filter(|(k, _)| k != "TERMINFO" && !set.iter().any(|(s, _)| k == s))
        .filter_map(|(k, v)| {
            let mut kv = k.into_encoded_bytes();
            kv.push(b'=');
            kv.extend(v.into_encoded_bytes());
            CString::new(kv).ok()
        })
        .collect();
    env.extend(set.iter().filter_map(|(k, v)| CString::new(format!("{k}={v}")).ok()));
    let mut envp: Vec<*const libc::c_char> = env.iter().map(|e| e.as_ptr()).collect();
    envp.push(std::ptr::null());
    let argv = [sh.as_ptr(), dash_c.as_ptr(), cmd.as_ptr(), std::ptr::null()];

    let pid = unsafe { libc::forkpty(&mut master, std::ptr::null_mut(), std::ptr::null_mut(), &ws as *const _ as *mut _) };
    if pid < 0 { return -1; }
    if pid == 0 {
        unsafe {
            // UTF-8 input: in canonical mode (`read`, `cat`) Backspace takes
            // a whole character off, not one byte of it.
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) == 0 {
                t.c_iflag |= libc::IUTF8;
                libc::tcsetattr(0, libc::TCSANOW, &t);
            }
            // sprid ignores SIGPIPE (see `sys::ignore_sigpipe`); an ignored
            // signal stays ignored across exec, and a shell's pipelines need
            // it back.
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            if let Some(dir) = dir.as_ref() {
                libc::chdir(dir.as_ptr());
            }
            libc::execve(sh.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::_exit(127);
        }
    }
    unsafe {
        // Not inherited by later tabs' programs, new windows or pbcopy: a
        // copy of the master held elsewhere would keep this tab's session
        // alive after the tab closes.
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        // Never block the window on a child that doesn't read: what doesn't
        // fit waits in PENDING.
        let flags = libc::fcntl(master, libc::F_GETFL);
        libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    start_reader(master);
    STARTED.with(|m| m.borrow_mut().insert(master, pid));
    // Asked when the app is told to quit: only with a program running is
    // there anything to ask the user about.
    crate::window::set_quit_check(any_busy);
    master as i64
}

/// Whether any shell sprid started is running a program.
fn any_busy() -> bool {
    let fds: Vec<i32> = STARTED.with(|m| m.borrow().keys().copied().collect());
    fds.into_iter().any(|fd| !busy(fd as i64).is_empty())
}

fn errno() -> i32 { std::io::Error::last_os_error().raw_os_error().unwrap_or(0) }

/// What `fd`'s program wrote since the last call — up to `READ_MAX` of it,
/// waiting up to `timeout_ms` when there is nothing yet; empty on timeout,
/// and once the child side has closed (`eof` then says so).
pub fn read(fd: i64, timeout_ms: i64) -> AlmideRcCow<Vec<u8>> {
    let Some(ch) = chan(fd) else { return read_direct(fd, timeout_ms) };
    let mut q = ch.queued.lock().unwrap();
    if q.data.is_empty() && !q.eof && timeout_ms > 0 {
        let (guard, _) = ch.changed.wait_timeout(q, std::time::Duration::from_millis(timeout_ms as u64)).unwrap();
        q = guard;
    }
    let out = if q.data.len() > READ_MAX {
        let rest = q.data.split_off(READ_MAX);
        std::mem::replace(&mut q.data, rest)
    } else {
        std::mem::take(&mut q.data)
    };
    if q.data.is_empty() {
        if q.eof { ENDED.with(|e| e.borrow_mut().insert(fd as i32)); }
        // Nothing left: the wake pipe emptied, so the loop sleeps again.
        if q.signalled && !q.eof {
            let mut sink = [0u8; 16];
            while unsafe { libc::read(ch.wake_r, sink.as_mut_ptr() as *mut _, sink.len()) } > 0 {}
            q.signalled = false;
        }
    }
    // The thread may wait for room.
    if !out.is_empty() { ch.changed.notify_all(); }
    AlmideRcCow::new(out)
}

/// Wait up to `timeout_ms` until any of `fds` has output (or has ended).
pub fn wait(fds: &[i64], timeout_ms: i64) {
    let mut pfds: Vec<libc::pollfd> =
        fds.iter().map(|&fd| libc::pollfd { fd: wake_fd(fd) as i32, events: libc::POLLIN, revents: 0 }).collect();
    if pfds.is_empty() { return; }
    unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout_ms as i32) };
}

/// `read` straight from the master, for one without a reader thread.
fn read_direct(fd: i64, timeout_ms: i64) -> AlmideRcCow<Vec<u8>> {
    let mut out = take(fd);
    if matches!(out, Err(libc::EAGAIN)) {
        let mut pfd = libc::pollfd { fd: fd as i32, events: libc::POLLIN, revents: 0 };
        let n = unsafe { libc::poll(&mut pfd, 1, timeout_ms as i32) };
        if n <= 0 { return AlmideRcCow::new(Vec::new()); }
        out = take(fd);
    }
    match out {
        Ok(v) => AlmideRcCow::new(v),
        // Interrupted, or nothing after all: not the end.
        Err(e) if e == libc::EINTR || e == libc::EAGAIN => AlmideRcCow::new(Vec::new()),
        // 0, or EIO once the child side is closed.
        Err(_) => {
            ENDED.with(|e| e.borrow_mut().insert(fd as i32));
            AlmideRcCow::new(Vec::new())
        }
    }
}

/// One read of `fd` into the scratch buffer; what came, or the errno (0 at
/// the end of the child's output). One scratch buffer for every read; what
/// comes back is a copy of just the bytes read. A fresh 64 KB per call,
/// zeroed, cost more than the parsing when macOS hands over a PTY 1 KB at a
/// time.
fn take(fd: i64) -> Result<Vec<u8>, i32> {
    SCRATCH.with(|b| {
        let mut buf = b.borrow_mut();
        let r = unsafe { libc::read(fd as i32, buf.as_mut_ptr() as *mut _, buf.len()) };
        if r > 0 { Ok(buf[..r as usize].to_vec()) } else { Err(if r == 0 { 0 } else { errno() }) }
    })
}

/// `true` once a read of `fd` has hit the end of the child's output.
pub fn eof(fd: i64) -> bool {
    ENDED.with(|e| e.borrow().contains(&(fd as i32)))
        || chan(fd).is_some_and(|c| { let q = c.queued.lock().unwrap(); q.eof && q.data.is_empty() })
}

/// Send `data` to the child: as much as it takes now, the rest queued for
/// `flush` — a big paste into a program that echoes it must not wait on the
/// program while the program waits on the window to read its echo. `false`
/// once the PTY is gone.
pub fn write(fd: i64, data: &str) -> bool {
    PENDING.with(|p| p.borrow_mut().entry(fd as i32).or_default().extend_from_slice(data.as_bytes()));
    flush(fd)
}

/// Write what is queued for `fd` until the PTY takes no more. `false` once
/// the PTY is gone.
pub fn flush(fd: i64) -> bool {
    PENDING.with(|p| {
        let mut map = p.borrow_mut();
        let Some(buf) = map.get_mut(&(fd as i32)) else { return true };
        let mut off = 0;
        let mut alive = true;
        while off < buf.len() {
            let w = unsafe { libc::write(fd as i32, buf[off..].as_ptr() as *const _, buf.len() - off) };
            if w > 0 {
                off += w as usize;
            } else {
                let e = errno();
                if e == libc::EINTR { continue; }
                alive = e == libc::EAGAIN;
                break;
            }
        }
        buf.drain(..off);
        if buf.is_empty() || !alive { map.remove(&(fd as i32)); }
        alive
    })
}

/// Whether input for `fd` still waits to be written.
pub fn pending(fd: i64) -> bool {
    PENDING.with(|p| p.borrow().get(&(fd as i32)).is_some_and(|b| !b.is_empty()))
}

/// Tell the child its window is now `cols` x `rows` (SIGWINCH follows).
pub fn resize(fd: i64, cols: i64, rows: i64) -> bool {
    let ws = libc::winsize { ws_row: rows as u16, ws_col: cols as u16, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(fd as i32, libc::TIOCSWINSZ, &ws) == 0 }
}

/// Close the master side: the child's session gets SIGHUP, as when a terminal
/// window closes.
pub fn close(fd: i64) {
    ENDED.with(|e| e.borrow_mut().remove(&(fd as i32)));
    PENDING.with(|p| p.borrow_mut().remove(&(fd as i32)));
    STARTED.with(|m| m.borrow_mut().remove(&(fd as i32)));
    let ch = chans().lock().unwrap().remove(&(fd as i32));
    match ch {
        // The thread closes the master (and `stop_r`) once told to stop.
        Some(ch) => {
            let _q = ch.queued.lock().unwrap();
            ch.stop.store(true, Ordering::Relaxed);
            let b = 1u8;
            unsafe {
                libc::write(ch.stop_w, &b as *const u8 as *const _, 1);
                libc::close(ch.wake_r);
                libc::close(ch.wake_w);
                libc::close(ch.stop_w);
            }
            ch.changed.notify_all();
        }
        None => unsafe { libc::close(fd as i32); },
    }
}

/// Collect children that have exited, so they don't linger as zombies.
pub fn reap() {
    let mut status = 0;
    while unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } > 0 {}
}

/// The working directory of the program in the foreground on `fd` — the
/// shell, or what it runs — as the system knows it; empty when unknown. What
/// a new tab starts in, whether or not the shell reports its directory.
pub fn cwd(fd: i64) -> String {
    #[cfg(target_os = "macos")]
    unsafe {
        let pgid = libc::tcgetpgrp(fd as i32);
        if pgid <= 0 { return String::new(); }
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
        let n = libc::proc_pidinfo(pgid, libc::PROC_PIDVNODEPATHINFO, 0, &mut info as *mut _ as *mut libc::c_void, size);
        if n != size { return String::new(); }
        let path = &info.pvi_cdir.vip_path;
        // libc spells the 1024-byte path as 32 rows of 32.
        let bytes: Vec<u8> = path.iter().flatten().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
        String::from_utf8(bytes).unwrap_or_default()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let pgid = unsafe { libc::tcgetpgrp(fd as i32) };
        if pgid <= 0 { return String::new(); }
        std::fs::read_link(format!("/proc/{pgid}/cwd")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// The name of a program the PTY's first program — the shell — is running
/// (an editor, a build), or "" when it runs none: it waits at its
/// prompt, or `fd` is unknown. What closing the tab would end without asking.
/// Asked of the shell's children rather than the terminal's foreground
/// group, which is only the command's when the shell does job control.
pub fn busy(fd: i64) -> String {
    let Some(shell) = STARTED.with(|m| m.borrow().get(&(fd as i32)).copied()) else { return String::new() };
    match children(shell).first() {
        Some(&child) => name_of(child).unwrap_or_else(|| "a program".to_string()),
        None => String::new(),
    }
}

/// The pid of the program `busy` names, 0 when the shell runs none.
pub fn busy_pid(fd: i64) -> i64 {
    let Some(shell) = STARTED.with(|m| m.borrow().get(&(fd as i32)).copied()) else { return 0 };
    children(shell).first().map(|&p| p as i64).unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn children(pid: i32) -> Vec<i32> {
    let mut buf = vec![0i32; 64];
    let n = unsafe { libc::proc_listchildpids(pid, buf.as_mut_ptr() as *mut _, (buf.len() * 4) as i32) };
    buf.truncate(n.max(0) as usize);
    buf.retain(|&p| p > 0);
    buf
}

#[cfg(not(target_os = "macos"))]
fn children(pid: i32) -> Vec<i32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
        .map(|s| s.split_whitespace().filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn name_of(pid: i32) -> Option<String> {
    let mut buf = [0u8; 256];
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr() as *mut _, buf.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

#[cfg(not(target_os = "macos"))]
fn name_of(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
}
