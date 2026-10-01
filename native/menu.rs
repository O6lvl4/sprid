//! The macOS menu bar. Backs `set_menu` in `src/gui/dialog.almd`; the
//! menus themselves are described in `src/gui/menubar.almd`.
//!
//! An item does what its key does: picking it hands sprid the same input
//! the key would (`crate::window::inject`), so the menu and the keyboard
//! can't come to differ. Its key equivalent is what macOS shows beside it —
//! and, as for every Mac app, a key the menu takes no longer reaches the
//! window, which is fine for the same reason.

/// Set the menu bar from `spec`: a menu per `>Title` line (`>Title*` the
/// Window menu, which lists the windows; `>Title?` the Help menu), then its
/// items, one per line — `-` a separator, else tab-separated fields:
///
///   title  key  modifiers  action
///
/// `key` is the key equivalent ("" none; `up` / `down` / `return` name keys
/// that type nothing), `modifiers` NSEventModifierFlags bits, and `action`
/// one of `key:<kind>:<code>:<mods>` (the input to hand sprid),
/// `sel:<selector>` (a standard action, sent along the responder chain) or
/// `open:<url>`. Nothing elsewhere than macOS.
pub fn set_menu(spec: &str) {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::set_menu(spec)
    }
    #[cfg(not(target_os = "macos"))]
    let _ = spec;
}

/// Pick item `item` of menu `menu` (both counted from 0, separators
/// included) as a click would: for a test of what the menus do.
pub fn perform(menu: i64, item: i64) {
    #[cfg(target_os = "macos")]
    unsafe {
        mac::perform(menu, item)
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (menu, item);
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void, CString};
    use std::sync::Mutex;

    type Id = *mut c_void;

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Id;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
        fn objc_registerClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Id, imp: *const c_void, types: *const c_char) -> bool;
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    unsafe fn sel(name: &str) -> Id {
        let c = CString::new(name).unwrap_or_default();
        unsafe { sel_registerName(c.as_ptr()) }
    }
    unsafe fn class(name: &str) -> Id {
        let c = CString::new(name).unwrap_or_default();
        unsafe { objc_getClass(c.as_ptr()) }
    }
    unsafe fn send(obj: Id, name: &str) -> Id {
        let f: unsafe extern "C" fn(Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name)) }
    }
    unsafe fn send_id(obj: Id, name: &str, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name), arg) }
    }
    unsafe fn send_uint(obj: Id, name: &str, arg: usize) {
        let f: unsafe extern "C" fn(Id, Id, usize) = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(obj, sel(name), arg) }
    }
    unsafe fn ns_string(s: &str) -> Id {
        let c = CString::new(s).unwrap_or_default();
        let f: unsafe extern "C" fn(Id, Id, *const c_char) -> Id = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr()) }
    }

    /// What each item does, by its tag.
    static ACTIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn act(_this: Id, _cmd: Id, sender: Id) {
        let f: unsafe extern "C" fn(Id, Id) -> isize = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        let tag = unsafe { f(sender, sel("tag")) };
        let action = ACTIONS.lock().ok().and_then(|a| a.get(tag as usize).cloned()).unwrap_or_default();
        if let Some(input) = action.strip_prefix("key:") {
            let n: Vec<i64> = input.split(':').filter_map(|x| x.parse().ok()).collect();
            if let [kind, code, mods] = n[..] {
                crate::window::inject(kind, code, mods);
            }
        } else if let Some(url) = action.strip_prefix("open:") {
            crate::sys::open_target(url);
        }
    }

    /// The object whose `act:` the items that hand sprid an input call.
    unsafe fn target() -> Id {
        static TARGET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        *TARGET.get_or_init(|| unsafe {
            let cls = objc_allocateClassPair(class("NSObject"), c"SpridMainMenu".as_ptr(), 0);
            class_addMethod(cls, sel("act:"), act as *const c_void, c"v@:@".as_ptr());
            objc_registerClassPair(cls);
            send(send(cls, "alloc"), "init") as usize
        }) as Id
    }

    /// NSUpArrowFunctionKey and the rest, as a key equivalent.
    fn key_equivalent(key: &str) -> String {
        match key {
            "up" => "\u{F700}".to_string(),
            "down" => "\u{F701}".to_string(),
            "return" => "\r".to_string(),
            k => k.to_string(),
        }
    }

    pub unsafe fn perform(menu: i64, item: i64) {
        unsafe {
            let app = send(class("NSApplication"), "sharedApplication");
            let at: unsafe extern "C" fn(Id, Id, isize) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let holder = at(send(app, "mainMenu"), sel("itemAtIndex:"), menu as isize);
            if holder.is_null() {
                return;
            }
            let sub = send(holder, "submenu");
            let go: unsafe extern "C" fn(Id, Id, isize) = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            go(sub, sel("performActionForItemAtIndex:"), item as isize);
        }
    }

    pub unsafe fn set_menu(spec: &str) {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let app = send(class("NSApplication"), "sharedApplication");
            let bar = send(send(class("NSMenu"), "alloc"), "init");
            let mut actions = Vec::new();
            let mut menu: Id = std::ptr::null_mut();
            for line in spec.lines() {
                if let Some(head) = line.strip_prefix('>') {
                    let (title, role) = match head.chars().last() {
                        Some('*') | Some('?') => (&head[..head.len() - 1], head.chars().last()),
                        _ => (head, None),
                    };
                    menu = send_id(send(class("NSMenu"), "alloc"), "initWithTitle:", ns_string(title));
                    let holder = send(send(class("NSMenuItem"), "alloc"), "init");
                    send_id(holder, "setSubmenu:", menu);
                    send_id(bar, "addItem:", holder);
                    match role {
                        Some('*') => { send_id(app, "setWindowsMenu:", menu); }
                        Some('?') => { send_id(app, "setHelpMenu:", menu); }
                        _ => {}
                    }
                    continue;
                }
                if menu.is_null() {
                    continue;
                }
                if line == "-" {
                    send_id(menu, "addItem:", send(class("NSMenuItem"), "separatorItem"));
                    continue;
                }
                let f: Vec<&str> = line.split('\t').collect();
                let [title, key, mask, action] = f[..] else { continue };
                let selector = match action.strip_prefix("sel:") {
                    Some(s) => sel(s),
                    None => sel("act:"),
                };
                let init: unsafe extern "C" fn(Id, Id, Id, Id, Id) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let item = init(send(class("NSMenuItem"), "alloc"), sel("initWithTitle:action:keyEquivalent:"),
                    ns_string(title), selector, ns_string(&key_equivalent(key)));
                send_uint(item, "setKeyEquivalentModifierMask:", mask.parse().unwrap_or(0));
                if !action.starts_with("sel:") {
                    send_id(item, "setTarget:", target());
                    send_uint(item, "setTag:", actions.len());
                    actions.push(action.to_string());
                }
                send_id(menu, "addItem:", item);
            }
            if let Ok(mut a) = ACTIONS.lock() {
                *a = actions;
            }
            send_id(app, "setMainMenu:", bar);
            objc_autoreleasePoolPop(pool);
        }
    }
}
