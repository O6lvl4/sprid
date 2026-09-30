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
}

/// Start `program` (run through `/bin/sh -c`) on a new PTY of `cols` x `rows`,
/// in directory `cwd` when it is not empty. Returns the master fd, or -1.
pub fn spawn(program: &str, cwd: &str, cols: i64, rows: i64) -> i64 {
    let ws = libc::winsize { ws_row: rows as u16, ws_col: cols as u16, ws_xpixel: 0, ws_ypixel: 0 };
    let mut master: libc::c_int = -1;
    let sh = std::ffi::CString::new("/bin/sh").unwrap();
    let dash_c = std::ffi::CString::new("-c").unwrap();
    let Ok(cmd) = std::ffi::CString::new(program) else { return -1 };
    let dir = std::ffi::CString::new(cwd).ok();
    let pid = unsafe { libc::forkpty(&mut master, std::ptr::null_mut(), std::ptr::null_mut(), &ws as *const _ as *mut _) };
    if pid < 0 { return -1; }
    if pid == 0 {
        // What the child sees of its terminal: xterm's sequences, direct colour.
        for (k, v) in [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "sprid")] {
            std::env::set_var(k, v);
        }
        std::env::remove_var("TERMINFO");
        unsafe {
            if let Some(dir) = dir.as_ref().filter(|d| !d.as_bytes().is_empty()) {
                libc::chdir(dir.as_ptr());
            }
            let argv = [sh.as_ptr(), dash_c.as_ptr(), cmd.as_ptr(), std::ptr::null()];
            libc::execv(sh.as_ptr(), argv.as_ptr());
            libc::_exit(127);
        }
    }
    master as i64
}

/// Wait up to `timeout_ms` for output on `fd` and return what is there; empty
/// on timeout, and once the child side has closed (`eof` then says so).
pub fn read(fd: i64, timeout_ms: i64) -> AlmideRcCow<Vec<u8>> {
    let mut pfd = libc::pollfd { fd: fd as i32, events: libc::POLLIN, revents: 0 };
    let n = unsafe { libc::poll(&mut pfd, 1, timeout_ms as i32) };
    if n <= 0 { return AlmideRcCow::new(Vec::new()); }
    let mut buf = vec![0u8; 65536];
    let r = unsafe { libc::read(fd as i32, buf.as_mut_ptr() as *mut _, buf.len()) };
    if r <= 0 {
        ENDED.with(|e| e.borrow_mut().insert(fd as i32));
        buf.clear();
    } else {
        buf.truncate(r as usize);
    }
    AlmideRcCow::new(buf)
}

/// Wait up to `timeout_ms` until any of `fds` has output (or has ended).
pub fn wait(fds: &[i64], timeout_ms: i64) {
    let mut pfds: Vec<libc::pollfd> =
        fds.iter().map(|&fd| libc::pollfd { fd: fd as i32, events: libc::POLLIN, revents: 0 }).collect();
    if pfds.is_empty() { return; }
    unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, timeout_ms as i32) };
}

/// `true` once a read of `fd` has hit the end of the child's output.
pub fn eof(fd: i64) -> bool { ENDED.with(|e| e.borrow().contains(&(fd as i32))) }

/// Write all of `data` to the PTY. `false` on error.
pub fn write(fd: i64, data: &str) -> bool {
    let data = data.as_bytes();
    let mut off = 0;
    while off < data.len() {
        let w = unsafe { libc::write(fd as i32, data[off..].as_ptr() as *const _, data.len() - off) };
        if w <= 0 { return false; }
        off += w as usize;
    }
    true
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
    unsafe { libc::close(fd as i32) };
}

/// Collect children that have exited, so they don't linger as zombies.
pub fn reap() {
    let mut status = 0;
    while unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } > 0 {}
}
