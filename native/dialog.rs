//! A question put to the user in the system's own dialog. Backs
//! `src/gui/dialog.almd`.

/// Ask with a warning alert: `message` in bold, `detail` under it, and the
/// buttons `ok` (the default, Return) and `cancel` (Escape). `true` for `ok`.
/// Blocks until answered. Elsewhere than macOS it answers `true`.
pub fn confirm(message: &str, detail: &str, ok: &str, cancel: &str) -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::confirm(message, detail, ok, cancel)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (message, detail, ok, cancel);
        true
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void, CString};

    type Id = *mut c_void;

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Id;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    unsafe fn sel(name: &str) -> Id {
        let c = CString::new(name).unwrap();
        unsafe { sel_registerName(c.as_ptr()) }
    }
    unsafe fn send(obj: Id, name: &str) -> Id {
        let f: unsafe extern "C" fn(Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name)) }
    }
    unsafe fn send_id(obj: Id, name: &str, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name), arg) }
    }
    unsafe fn send_int(obj: Id, name: &str, arg: isize) -> Id {
        let f: unsafe extern "C" fn(Id, Id, isize) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name), arg) }
    }
    unsafe fn run_modal(obj: Id) -> isize {
        let f: unsafe extern "C" fn(Id, Id) -> isize = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel("runModal")) }
    }
    unsafe fn ns_string(s: &str) -> Id {
        let c = CString::new(s.replace('\0', "")).unwrap();
        let class = unsafe { objc_getClass(c"NSString".as_ptr()) };
        let f: unsafe extern "C" fn(Id, Id, *const c_char) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(class, sel("stringWithUTF8String:"), c.as_ptr()) }
    }

    /// NSAlertFirstButtonReturn.
    const FIRST: isize = 1000;
    /// NSAlertStyleWarning.
    const WARNING: isize = 0;

    pub unsafe fn confirm(message: &str, detail: &str, ok: &str, cancel: &str) -> bool {
        unsafe {
            // The strings below are autoreleased; this is called between
            // event pumps, outside any pool that would release them.
            let pool = objc_autoreleasePoolPush();
            let answer = ask(message, detail, ok, cancel);
            objc_autoreleasePoolPop(pool);
            answer
        }
    }

    unsafe fn ask(message: &str, detail: &str, ok: &str, cancel: &str) -> bool {
        unsafe {
            let alert = send(send(objc_getClass(c"NSAlert".as_ptr()), "alloc"), "init");
            send_int(alert, "setAlertStyle:", WARNING);
            send_id(alert, "setMessageText:", ns_string(message));
            send_id(alert, "setInformativeText:", ns_string(detail));
            send_id(alert, "addButtonWithTitle:", ns_string(ok));
            let cancel_button = send_id(alert, "addButtonWithTitle:", ns_string(cancel));
            // Escape answers Cancel.
            send_id(cancel_button, "setKeyEquivalent:", ns_string("\u{1b}"));
            let answer = run_modal(alert);
            send(alert, "release");
            answer == FIRST
        }
    }
}
