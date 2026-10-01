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

/// The system's alert sound. Nothing elsewhere than macOS.
pub fn beep() {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::NSBeep()
    }
}

/// A menu of `items` at the pointer — one per line of `items`, a line
/// starting with `!` shown disabled, `-` a separator — and which was
/// picked: its index, or -1 for none. Blocks until it closes. Elsewhere
/// than macOS there is no menu: -1.
pub fn context_menu(items: &str) -> i64 {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::context_menu(items)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = items;
        -1
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
    extern "C" {
        pub fn NSBeep();
    }

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

    #[link(name = "objc")]
    extern "C" {
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
        fn objc_registerClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Id, imp: *const c_void, types: *const c_char) -> bool;
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: f64,
        y: f64,
    }

    /// The tag of the item picked from the last menu, -1 for none.
    static PICKED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);

    unsafe extern "C" fn pick(_this: Id, _cmd: Id, sender: Id) {
        let f: unsafe extern "C" fn(Id, Id) -> isize = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        let tag = unsafe { f(sender, sel("tag")) };
        PICKED.store(tag as i64, std::sync::atomic::Ordering::Relaxed);
    }

    /// An object whose `pick:` notes the sender's tag, the menu items' target.
    unsafe fn target() -> Id {
        static CLASS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        let cls = *CLASS.get_or_init(|| unsafe {
            let cls = objc_allocateClassPair(objc_getClass(c"NSObject".as_ptr()), c"SpridMenuTarget".as_ptr(), 0);
            class_addMethod(cls, sel("pick:"), pick as *const c_void, c"v@:@".as_ptr());
            objc_registerClassPair(cls);
            cls as usize
        });
        unsafe { send(send(cls as Id, "alloc"), "init") }
    }

    pub unsafe fn context_menu(items: &str) -> i64 {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let picked = popup(items);
            objc_autoreleasePoolPop(pool);
            picked
        }
    }

    unsafe fn popup(items: &str) -> i64 {
        unsafe {
            PICKED.store(-1, std::sync::atomic::Ordering::Relaxed);
            let menu = send(send(objc_getClass(c"NSMenu".as_ptr()), "alloc"), "init");
            let set_bool: unsafe extern "C" fn(Id, Id, bool) = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            set_bool(menu, sel("setAutoenablesItems:"), false);
            let target = target();
            for (i, line) in items.lines().enumerate() {
                if line == "-" {
                    send_id(menu, "addItem:", send(objc_getClass(c"NSMenuItem".as_ptr()), "separatorItem"));
                    continue;
                }
                let (title, enabled) = match line.strip_prefix('!') {
                    Some(t) => (t, false),
                    None => (line, true),
                };
                let init: unsafe extern "C" fn(Id, Id, Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let item = init(send(objc_getClass(c"NSMenuItem".as_ptr()), "alloc"), sel("initWithTitle:action:keyEquivalent:"),
                    ns_string(title), sel("pick:"), ns_string(""));
                send_id(item, "setTarget:", target);
                send_int(item, "setTag:", i as isize);
                set_bool(item, sel("setEnabled:"), enabled);
                send_id(menu, "addItem:", item);
                send(item, "release");
            }
            let at: unsafe extern "C" fn(Id, Id) -> Point = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let location = at(objc_getClass(c"NSEvent".as_ptr()), sel("mouseLocation"));
            let pop: unsafe extern "C" fn(Id, Id, Id, Point, Id) -> bool = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            pop(menu, sel("popUpMenuPositioningItem:atLocation:inView:"), std::ptr::null_mut(), location, std::ptr::null_mut());
            send(menu, "release");
            send(target, "release");
            PICKED.load(std::sync::atomic::Ordering::Relaxed)
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
