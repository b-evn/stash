#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(windows)]
pub use win::*;
#[cfg(not(windows))]
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

    pub fn force_foreground(raw: isize) {
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
}

#[cfg(not(windows))]
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
    pub fn force_foreground(_raw: isize) {}
    pub fn paste_into(_target: isize) {}
    pub fn clipboard_is_sensitive() -> bool {
        false
    }

    pub struct PasteWatch;
    pub fn watch_pastes(_tx: std::sync::mpsc::Sender<()>) -> PasteWatch {
        PasteWatch
    }
}
