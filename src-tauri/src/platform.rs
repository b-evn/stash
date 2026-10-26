#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

// Screen coordinates are physical pixels on Windows and points on macOS (see `lib.rs`).
// A target is the window (Windows) or process id (macOS) that had focus when the popup opened.

#[cfg(windows)]
pub use win::*;
#[cfg(target_os = "macos")]
pub use mac::*;
#[cfg(not(any(windows, target_os = "macos")))]
pub use other::*;

#[cfg(windows)]
mod win {
    use super::Rect;
    use std::{ffi::c_void, mem::size_of, thread::sleep, time::Duration};
    use windows::{
        core::w,
        Win32::{
            Foundation::{HWND, POINT},
            Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST},
            System::{
                DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW},
                Threading::GetCurrentThreadId,
            },
            UI::{
                Input::KeyboardAndMouse::{
                    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
                },
                WindowsAndMessaging::{
                    BringWindowToTop, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo,
                    GetWindowThreadProcessId, IsWindow, SetForegroundWindow, GUITHREADINFO,
                },
            },
        },
    };

    fn hwnd(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    pub fn foreground() -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }

    pub fn caret_or_cursor() -> (i32, i32, i32) {
        unsafe {
            let fg = GetForegroundWindow();
            if !fg.0.is_null() {
                let thread = GetWindowThreadProcessId(fg, None);
                let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
                if GetGUIThreadInfo(thread, &mut info).is_ok() && !info.hwndCaret.0.is_null() {
                    let r = info.rcCaret;
                    let mut top = POINT { x: r.left, y: r.top };
                    let mut bottom = POINT { x: r.left, y: r.bottom.max(r.top + 16) };
                    if ClientToScreen(info.hwndCaret, &mut top).as_bool()
                        && ClientToScreen(info.hwndCaret, &mut bottom).as_bool()
                    {
                        return (top.x, top.y, bottom.y);
                    }
                }
            }
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            (p.x, p.y, p.y + 20)
        }
    }

    pub fn work_area(x: i32, y: i32) -> Rect {
        unsafe {
            let mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
            if GetMonitorInfoW(mon, &mut mi).as_bool() {
                let r = mi.rcWork;
                return Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
            }
            Rect { left: 0, top: 0, right: 1920, bottom: 1040 }
        }
    }

    pub fn request_access() {}
    pub fn prepare_popup(_w: &tauri::WebviewWindow) {}
    pub fn release_focus(_w: &tauri::WebviewWindow) {}
    pub fn watch_screenshots(_on_new: impl Fn(&std::path::Path) + Send + 'static) {}
    pub fn copy_image_file(_path: &std::path::Path) -> bool {
        false
    }

    pub fn focus_popup(w: &tauri::WebviewWindow) {
        if let Ok(h) = w.hwnd() {
            force_foreground(h.0 as isize);
        }
    }

    fn force_foreground(raw: isize) {
        unsafe {
            let target = hwnd(raw);
            let current = GetCurrentThreadId();
            let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let attached = fg_thread != 0
                && fg_thread != current
                && windows::Win32::System::Threading::AttachThreadInput(current, fg_thread, true).as_bool();
            let _ = BringWindowToTop(target);
            let _ = SetForegroundWindow(target);
            if attached {
                let _ = windows::Win32::System::Threading::AttachThreadInput(current, fg_thread, false);
            }
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs, size_of::<INPUT>() as i32);
        }
    }

    pub fn paste_into(target: isize) {
        if target == 0 {
            return;
        }
        unsafe {
            let h = hwnd(target);
            if !IsWindow(Some(h)).as_bool() {
                return;
            }
            let _ = SetForegroundWindow(h);
            for _ in 0..50 {
                if GetForegroundWindow() == h {
                    break;
                }
                sleep(Duration::from_millis(10));
            }
            sleep(Duration::from_millis(40));

            let held: Vec<INPUT> = [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
                .into_iter()
                .filter(|vk| GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0)
                .map(|vk| key(vk, true))
                .collect();
            if !held.is_empty() {
                send(&held);
            }
            send(&[key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)]);
        }
    }

    use std::sync::{mpsc::Sender, Mutex};
    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
            LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
        },
    };

    static PASTED: Mutex<Option<Sender<()>>> = Mutex::new(None);

    unsafe extern "system" fn keyboard_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 && (w.0 as u32 == WM_KEYUP || w.0 as u32 == WM_SYSKEYUP) {
            let k = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let down = |vk: VIRTUAL_KEY| GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0;
            if k.vkCode == VK_V.0 as u32 && k.flags.0 & LLKHF_INJECTED.0 == 0 && down(VK_CONTROL) && !down(VK_MENU) {
                if let Some(tx) = PASTED.lock().ok().and_then(|g| g.clone()) {
                    let _ = tx.send(());
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    pub struct PasteWatch {
        thread: u32,
    }

    pub fn watch_pastes(tx: Sender<()>) -> PasteWatch {
        *PASTED.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        let (id_tx, id_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || unsafe {
            let _ = id_tx.send(GetCurrentThreadId());
            let module = GetModuleHandleW(None).ok().map(|m| m.into());
            let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) else { return };
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
            let _ = UnhookWindowsHookEx(hook);
        });
        PasteWatch { thread: id_rx.recv().unwrap_or(0) }
    }

    impl Drop for PasteWatch {
        fn drop(&mut self) {
            *PASTED.lock().unwrap_or_else(|e| e.into_inner()) = None;
            unsafe {
                let _ = PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn clipboard_is_sensitive() -> bool {
        unsafe {
            let fmt = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            fmt != 0 && IsClipboardFormatAvailable(fmt).is_ok()
        }
    }

    pub fn reveal(path: &str) {
        use std::process::Command;
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        let arg = if std::path::Path::new(path).is_dir() {
            path.to_string()
        } else {
            format!("/select,{path}")
        };
        let _ = Command::new("explorer.exe").arg(arg).spawn();
    }

    pub fn open(target: &str) {
        use windows::core::HSTRING;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let target = target.trim();
        if target.is_empty() {
            return;
        }
        unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(target),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Rect;
    use objc2::{rc::autoreleasepool, runtime::ProtocolObject, AnyThread, MainThreadMarker};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationOptions, NSImage, NSPasteboard, NSPopUpMenuWindowLevel,
        NSRunningApplication, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWorkspace,
    };
    use objc2_application_services::{
        kAXTrustedCheckOptionPrompt, AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement, AXValue,
        AXValueType,
    };
    use objc2_core_foundation::{
        kCFBooleanTrue, kCFRunLoopDefaultMode, CFDictionary, CFMachPort, CFPreferencesAppSynchronize,
        CFPreferencesCopyAppValue, CFRange, CFRetained, CFRunLoop, CFString, CFType, CGRect,
    };
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
        CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
    };
    use objc2_foundation::{NSArray, NSString, NSURL};
    use std::{
        cell::Cell,
        collections::HashSet,
        ffi::{c_void, CString, OsString},
        os::unix::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr::{self, NonNull},
        time::SystemTime,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::Sender,
            Arc,
        },
        thread::sleep,
        time::Duration,
    };

    // kVK_ANSI_V: the V key on US, German, French and most other layouts.
    const KEY_V: u16 = 9;
    // Tags the key events stash posts, so the paste watcher can skip them.
    const OWN_EVENT: i64 = 0x5354_4153;

    fn trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    // Pasting, the caret position and the paste stack need Accessibility access.
    // AXIsProcessTrustedWithOptions shows the system prompt on every call while
    // stash isn't trusted, so ask at most once per run and check silently after.
    pub fn request_access() {
        static ASKED: AtomicBool = AtomicBool::new(false);
        if trusted() || ASKED.swap(true, Ordering::Relaxed) {
            return;
        }
        let Some(yes) = (unsafe { kCFBooleanTrue }) else { return };
        let opts = CFDictionary::<CFString, CFType>::from_slices(&[unsafe { kAXTrustedCheckOptionPrompt }], &[yes]);
        unsafe { AXIsProcessTrustedWithOptions(Some(opts.as_opaque())) };
    }

    pub fn foreground() -> isize {
        let me = std::process::id() as i32;
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|a| a.processIdentifier())
            .filter(|&pid| pid != me)
            .unwrap_or(0) as isize
    }

    fn ax_copy(el: &AXUIElement, attr: &str, param: Option<&CFType>) -> Option<CFRetained<CFType>> {
        let name = CFString::from_str(attr);
        let mut out: *const CFType = ptr::null();
        let err = unsafe {
            match param {
                Some(p) => el.copy_parameterized_attribute_value(&name, p, NonNull::from(&mut out)),
                None => el.copy_attribute_value(&name, NonNull::from(&mut out)),
            }
        };
        let out = NonNull::new(out as *mut CFType)?;
        let out = unsafe { CFRetained::from_raw(out) };
        (err == AXError::Success).then_some(out)
    }

    fn bounds_for(el: &AXUIElement, location: isize, length: isize) -> Option<CGRect> {
        let mut range = CFRange { location, length };
        let param = unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&mut range).cast()) }?;
        let value = ax_copy(el, "AXBoundsForRange", Some(&param))?;
        let mut rect = CGRect::default();
        let ok = unsafe { value.downcast_ref::<AXValue>()?.value(AXValueType::CGRect, NonNull::from(&mut rect).cast()) };
        let sane = rect.size.height > 0.0 && rect.size.height < 400.0 && (rect.origin.x, rect.origin.y) != (0.0, 0.0);
        (ok && sane).then_some(rect)
    }

    // The text caret of the focused app through the Accessibility API, in global top-left points.
    fn caret() -> Option<(f64, f64, f64)> {
        if !trusted() {
            return None;
        }
        let system = unsafe { AXUIElement::new_system_wide() };
        unsafe { system.set_messaging_timeout(0.25) };
        let focused = ax_copy(&system, "AXFocusedUIElement", None)?;
        let focused = focused.downcast_ref::<AXUIElement>()?;
        let selection = ax_copy(focused, "AXSelectedTextRange", None)?;
        let mut range = CFRange { location: 0, length: 0 };
        let ok = unsafe {
            selection.downcast_ref::<AXValue>()?.value(AXValueType::CFRange, NonNull::from(&mut range).cast())
        };
        if !ok {
            return None;
        }
        if let Some(r) = bounds_for(focused, range.location, 0) {
            return Some((r.origin.x, r.origin.y, r.origin.y + r.size.height));
        }
        // Some apps only report bounds for real characters: use the right edge of the previous one.
        let r = bounds_for(focused, range.location.checked_sub(1)?, 1)?;
        Some((r.origin.x + r.size.width, r.origin.y, r.origin.y + r.size.height))
    }

    pub fn caret_or_cursor() -> (i32, i32, i32) {
        if let Some((x, top, bottom)) = caret() {
            return (x as i32, top as i32, bottom.max(top + 16.0) as i32);
        }
        let p = CGEvent::new(None).map(|e| CGEvent::location(Some(&e))).unwrap_or_default();
        (p.x as i32, p.y as i32, p.y as i32 + 20)
    }

    pub fn work_area(x: i32, y: i32) -> Rect {
        let fallback = Rect { left: 0, top: 0, right: 1440, bottom: 900 };
        let Some(mtm) = MainThreadMarker::new() else { return fallback };
        let screens = NSScreen::screens(mtm);
        let Some(primary) = screens.firstObject() else { return fallback };
        // Cocoa puts the origin at the bottom-left of the primary screen, y going up.
        let flip = primary.frame().size.height;
        let (px, py) = (x as f64, flip - y as f64);
        let screen = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                px >= f.origin.x && px < f.origin.x + f.size.width && py > f.origin.y && py <= f.origin.y + f.size.height
            })
            .unwrap_or(primary);
        let v = screen.visibleFrame();
        Rect {
            left: v.origin.x as i32,
            top: (flip - v.origin.y - v.size.height) as i32,
            right: (v.origin.x + v.size.width) as i32,
            bottom: (flip - v.origin.y) as i32,
        }
    }

    fn ns_window(w: &tauri::WebviewWindow) -> Option<&NSWindow> {
        MainThreadMarker::new()?;
        let ptr = w.ns_window().ok()? as *const NSWindow;
        unsafe { ptr.as_ref() }
    }

    // Float above full-screen apps and follow the user across Spaces.
    pub fn prepare_popup(w: &tauri::WebviewWindow) {
        let Some(win) = ns_window(w) else { return };
        win.setLevel(NSPopUpMenuWindowLevel);
        win.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }

    pub fn focus_popup(_w: &tauri::WebviewWindow) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        app.unhideWithoutActivation();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }

    // Hiding the window alone would leave stash active with nothing to type into;
    // hiding the app hands focus back to the app the user came from.
    pub fn release_focus(w: &tauri::WebviewWindow) {
        let Some(win) = ns_window(w) else {
            let w2 = w.clone();
            let _ = w.run_on_main_thread(move || release_focus(&w2));
            return;
        };
        win.orderOut(None);
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).hide(None);
        }
    }

    fn post_paste() {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState);
        for down in [true, false] {
            let Some(e) = CGEvent::new_keyboard_event(source.as_deref(), KEY_V, down) else { return };
            CGEvent::set_flags(Some(&e), CGEventFlags::MaskCommand);
            CGEvent::set_integer_value_field(Some(&e), CGEventField::EventSourceUserData, OWN_EVENT);
            CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&e));
        }
    }

    pub fn paste_into(target: isize) {
        if target <= 0 {
            return;
        }
        if !trusted() {
            request_access();
            return;
        }
        let pid = target as i32;
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else { return };
        if app.isTerminated() {
            return;
        }
        if !app.isActive() {
            app.activateWithOptions(NSApplicationActivationOptions::empty());
        }
        let front = || NSWorkspace::sharedWorkspace().frontmostApplication().map(|a| a.processIdentifier());
        for _ in 0..50 {
            if front() == Some(pid) {
                break;
            }
            sleep(Duration::from_millis(10));
        }
        sleep(Duration::from_millis(40));
        post_paste();
    }

    struct Tap {
        tx: Sender<()>,
        armed: Cell<bool>,
        port: Cell<*const CFMachPort>,
    }

    // Cmd+V down arms, V up fires: the app has read the clipboard by then.
    unsafe extern "C-unwind" fn on_key(
        _proxy: CGEventTapProxy,
        kind: CGEventType,
        event: NonNull<CGEvent>,
        info: *mut c_void,
    ) -> *mut CGEvent {
        let tap = unsafe { &*(info as *const Tap) };
        let e = Some(unsafe { event.as_ref() });
        if kind == CGEventType::TapDisabledByTimeout || kind == CGEventType::TapDisabledByUserInput {
            if let Some(port) = unsafe { tap.port.get().as_ref() } {
                CGEvent::tap_enable(port, true);
            }
        } else if CGEvent::integer_value_field(e, CGEventField::KeyboardEventKeycode) == KEY_V as i64
            && CGEvent::integer_value_field(e, CGEventField::EventSourceUserData) != OWN_EVENT
        {
            if kind == CGEventType::KeyDown {
                let flags = CGEvent::flags(e);
                tap.armed.set(
                    flags.contains(CGEventFlags::MaskCommand)
                        && !flags.intersects(CGEventFlags::MaskAlternate | CGEventFlags::MaskControl),
                );
            } else if kind == CGEventType::KeyUp && tap.armed.replace(false) {
                let _ = tap.tx.send(());
            }
        }
        event.as_ptr()
    }

    pub struct PasteWatch {
        stop: Arc<AtomicBool>,
    }

    pub fn watch_pastes(tx: Sender<()>) -> PasteWatch {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::spawn(move || {
            // Without Accessibility, creating a keyboard tap makes macOS ask for Input Monitoring.
            if !trusted() {
                eprintln!("stash: can't watch for pastes, allow stash under Privacy & Security > Accessibility");
                request_access();
                return;
            }
            let tap = Tap { tx, armed: Cell::new(false), port: Cell::new(ptr::null()) };
            let mask = (1u64 << CGEventType::KeyDown.0) | (1u64 << CGEventType::KeyUp.0);
            let port = unsafe {
                CGEvent::tap_create(
                    CGEventTapLocation::SessionEventTap,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    mask,
                    Some(on_key),
                    &tap as *const Tap as *mut c_void,
                )
            };
            let Some(port) = port else {
                eprintln!("stash: can't watch for pastes, allow stash under Privacy & Security > Accessibility");
                request_access();
                return;
            };
            tap.port.set(&*port);
            let (Some(source), Some(run_loop)) = (CFMachPort::new_run_loop_source(None, Some(&port), 0), CFRunLoop::current())
            else {
                return;
            };
            let mode = unsafe { kCFRunLoopDefaultMode };
            run_loop.add_source(Some(&source), mode);
            CGEvent::tap_enable(&port, true);
            while !stopped.load(Ordering::Relaxed) {
                CFRunLoop::run_in_mode(mode, 0.5, false);
            }
            CGEvent::tap_enable(&port, false);
            port.invalidate();
        });
        PasteWatch { stop }
    }

    impl Drop for PasteWatch {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    // Password managers mark secrets with these types (nspasteboard.org).
    pub fn clipboard_is_sensitive() -> bool {
        const MARKERS: [&str; 5] = [
            "org.nspasteboard.ConcealedType",
            "org.nspasteboard.TransientType",
            "org.nspasteboard.AutoGeneratedType",
            "com.agilebits.onepassword",
            "de.petermaurer.TransientPasteboardType",
        ];
        autoreleasepool(|_| {
            NSPasteboard::generalPasteboard()
                .types()
                .is_some_and(|types| types.iter().any(|t| MARKERS.contains(&t.to_string().as_str())))
        })
    }

    fn expand_home(path: &str) -> String {
        match (path.strip_prefix("~/"), std::env::var("HOME")) {
            (Some(rest), Ok(home)) => format!("{home}/{rest}"),
            _ => path.to_string(),
        }
    }

    // Where the screenshot tool saves: its "location" setting, else the Desktop.
    fn screenshot_dir() -> Option<PathBuf> {
        let domain = CFString::from_static_str("com.apple.screencapture");
        CFPreferencesAppSynchronize(&domain);
        let custom = CFPreferencesCopyAppValue(&CFString::from_static_str("location"), &domain)
            .and_then(|v| v.downcast::<CFString>().ok())
            .map(|s| expand_home(&s.to_string()));
        Some(PathBuf::from(custom.unwrap_or(format!("{}/Desktop", std::env::var("HOME").ok()?))))
    }

    // The screenshot tool tags its files, whatever the language of their names.
    fn is_screenshot(path: &Path) -> bool {
        let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return false };
        let tag = c"com.apple.metadata:kMDItemIsScreenCapture";
        unsafe { libc::getxattr(c.as_ptr(), tag.as_ptr(), ptr::null_mut(), 0, 0, 0) >= 0 }
    }

    fn is_image(path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "heic" | "tif" | "tiff")
    }

    fn file_names(dir: &Path) -> HashSet<OsString> {
        std::fs::read_dir(dir).map(|it| it.flatten().map(|e| e.file_name()).collect()).unwrap_or_default()
    }

    fn modified(dir: &Path) -> Option<SystemTime> {
        std::fs::metadata(dir).and_then(|m| m.modified()).ok()
    }

    // Checks the screenshot folder's timestamp four times a second and reports each new screenshot.
    pub fn watch_screenshots(on_new: impl Fn(&Path) + Send + 'static) {
        std::thread::spawn(move || {
            let (mut dir, mut stamp, mut seen) = (PathBuf::new(), None, HashSet::new());
            let mut tick = 0u64;
            loop {
                tick += 1;
                if tick % 40 == 1 {
                    if let Some(d) = screenshot_dir().filter(|d| *d != dir) {
                        (stamp, seen) = (modified(&d), file_names(&d));
                        dir = d;
                    }
                }
                sleep(Duration::from_millis(250));
                let now = modified(&dir);
                if now == stamp {
                    continue;
                }
                stamp = now;
                let names = file_names(&dir);
                for name in names.difference(&seen) {
                    let path = dir.join(name);
                    if name.as_bytes().starts_with(b".") || !is_image(&path) {
                        continue;
                    }
                    // The tag can land just after the file does.
                    for _ in 0..10 {
                        if is_screenshot(&path) {
                            on_new(&path);
                            break;
                        }
                        sleep(Duration::from_millis(200));
                    }
                }
                seen = names;
            }
        });
    }

    pub fn copy_image_file(path: &Path) -> bool {
        let Some(path) = path.to_str() else { return false };
        autoreleasepool(|_| {
            let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(path)) else {
                return false;
            };
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            pb.writeObjects(&NSArray::from_slice(&[ProtocolObject::from_ref(&*image)]))
        })
    }

    pub fn reveal(path: &str) {
        let path = expand_home(path.trim());
        if path.is_empty() {
            return;
        }
        autoreleasepool(|_| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
            let ws = NSWorkspace::sharedWorkspace();
            if std::path::Path::new(&path).is_dir() {
                ws.openURL(&url);
            } else {
                ws.activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
            }
        });
    }

    pub fn open(target: &str) {
        let target = target.trim();
        if target.is_empty() {
            return;
        }
        autoreleasepool(|_| {
            if let Some(url) = NSURL::URLWithString(&NSString::from_str(target)) {
                NSWorkspace::sharedWorkspace().openURL(&url);
            }
        });
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod other {
    use super::Rect;

    pub fn foreground() -> isize {
        0
    }
    pub fn caret_or_cursor() -> (i32, i32, i32) {
        (200, 200, 220)
    }
    pub fn work_area(_x: i32, _y: i32) -> Rect {
        Rect { left: 0, top: 0, right: 1920, bottom: 1040 }
    }
    pub fn request_access() {}
    pub fn prepare_popup(_w: &tauri::WebviewWindow) {}
    pub fn focus_popup(_w: &tauri::WebviewWindow) {}
    pub fn release_focus(_w: &tauri::WebviewWindow) {}
    pub fn watch_screenshots(_on_new: impl Fn(&std::path::Path) + Send + 'static) {}
    pub fn copy_image_file(_path: &std::path::Path) -> bool {
        false
    }
    pub fn paste_into(_target: isize) {}
    pub fn clipboard_is_sensitive() -> bool {
        false
    }
    pub fn reveal(path: &str) {
        open(if std::path::Path::new(path.trim()).is_dir() {
            path.trim()
        } else {
            std::path::Path::new(path.trim()).parent().and_then(|p| p.to_str()).unwrap_or(path.trim())
        });
    }
    pub fn open(target: &str) {
        let target = target.trim();
        if target.is_empty() {
            return;
        }
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }

    pub struct PasteWatch;
    pub fn watch_pastes(_tx: std::sync::mpsc::Sender<()>) -> PasteWatch {
        PasteWatch
    }
}
