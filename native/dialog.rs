//! A question put to the user in the system's own dialog, and desktop
//! notifications. Backs `src/gui/dialog.almd`.

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

/// Show a desktop notification: in Notification Center on macOS (asking
/// once for permission), through `notify-send` elsewhere. Never waits.
pub fn notify(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::notify(title, body)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("--app-name=sprid").arg(if title.is_empty() { "sprid" } else { title });
        if !body.is_empty() {
            cmd.arg(body);
        }
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        if let Ok(mut child) = cmd.spawn() {
            // Reaped on a thread of its own, so it never lingers as a zombie.
            std::thread::spawn(move || child.wait());
        }
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

    #[link(name = "UserNotifications", kind = "framework")]
    extern "C" {
        static _NSConcreteGlobalBlock: c_void;
    }

    /// A block taking (BOOL, NSError *) and doing nothing: the completion
    /// handler `requestAuthorization` must be given.
    #[repr(C)]
    struct Block {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: unsafe extern "C" fn(*const Block, bool, Id),
        descriptor: *const BlockDescriptor,
    }
    #[repr(C)]
    struct BlockDescriptor {
        reserved: usize,
        size: usize,
    }
    unsafe extern "C" fn ignore(_: *const Block, _: bool, _: Id) {}
    /// BLOCK_IS_GLOBAL.
    const GLOBAL: i32 = 1 << 28;

    static ASKED: std::sync::Once = std::sync::Once::new();

    pub unsafe fn notify(title: &str, body: &str) {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            post(title, body);
            objc_autoreleasePoolPop(pool);
        }
    }

    unsafe fn post(title: &str, body: &str) {
        unsafe {
            // Notification Center serves only an app bundle; sprid run as a
            // bare binary (a build in the source tree) would be killed by it.
            let bundle = send(objc_getClass(c"NSBundle".as_ptr()), "mainBundle");
            if send(bundle, "bundleIdentifier").is_null() {
                return;
            }
            let center = send(objc_getClass(c"UNUserNotificationCenter".as_ptr()), "currentNotificationCenter");
            if center.is_null() {
                return;
            }
            ASKED.call_once(|| {
                let descriptor: &'static BlockDescriptor =
                    Box::leak(Box::new(BlockDescriptor { reserved: 0, size: std::mem::size_of::<Block>() }));
                let block: &'static Block = Box::leak(Box::new(Block {
                    isa: &raw const _NSConcreteGlobalBlock,
                    flags: GLOBAL,
                    reserved: 0,
                    invoke: ignore,
                    descriptor,
                }));
                // UNAuthorizationOptionSound | UNAuthorizationOptionAlert.
                let f: unsafe extern "C" fn(Id, Id, usize, *const Block) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                f(center, sel("requestAuthorizationWithOptions:completionHandler:"), (1 << 1) | (1 << 2), block);
            });
            let content = send(send(objc_getClass(c"UNMutableNotificationContent".as_ptr()), "alloc"), "init");
            send_id(content, "setTitle:", ns_string(if title.is_empty() { "sprid" } else { title }));
            send_id(content, "setBody:", ns_string(body));
            send_id(content, "setSound:", send(objc_getClass(c"UNNotificationSound".as_ptr()), "defaultSound"));
            let id = send(send(objc_getClass(c"NSUUID".as_ptr()), "UUID"), "UUIDString");
            let f: unsafe extern "C" fn(Id, Id, Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let request = f(
                objc_getClass(c"UNNotificationRequest".as_ptr()),
                sel("requestWithIdentifier:content:trigger:"),
                id,
                content,
                std::ptr::null_mut(),
            );
            let add: unsafe extern "C" fn(Id, Id, Id, Id) = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            add(center, sel("addNotificationRequest:withCompletionHandler:"), request, std::ptr::null_mut());
            send(content, "release");
        }
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
