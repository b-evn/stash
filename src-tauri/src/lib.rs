mod capture;
mod config;
mod db;
mod kind;
mod ocr;
mod platform;

use std::{
    collections::VecDeque,
    io,
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        mpsc::{channel, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Position, Size,
    State, WebviewWindow, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use db::{Clip, Db};
use serde::Serialize;

const CARD_W: f64 = 384.0;
const MARGIN: f64 = 16.0;
const MIN_H: f64 = 120.0;
const MAX_H: f64 = 520.0;
const GAP: f64 = 4.0;
const ECHO_MS: i64 = 600;
const BLUR_GRACE: Duration = Duration::from_millis(250);
const CONFIG_POLL: Duration = Duration::from_millis(500);

struct Placement {
    x: i32,
    caret_top: i32,
    caret_bottom: i32,
    work: platform::Rect,
    above: bool,
    margin: i32,
    gap: i32,
}

// The clip at the front of the queue is always the one on the clipboard.
#[derive(Default)]
struct Stack {
    queue: VecDeque<i64>,
    done: usize,
    watch: Option<platform::PasteWatch>,
}

#[derive(Serialize, Clone)]
struct StackStatus {
    ids: Vec<i64>,
    done: usize,
}

struct AppState {
    stack: Mutex<Stack>,
    pasted: Sender<()>,
    db: Arc<Db>,
    quiet_until: Arc<AtomicI64>,
    target: Mutex<isize>,
    placement: Mutex<Option<Placement>>,
    height_px: Mutex<i32>,
    shown_at: Mutex<Instant>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn popup(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

// `platform` measures the screen in physical pixels on Windows and in points on macOS.
const POINTS: bool = cfg!(target_os = "macos");

fn screen_scale(w: &WebviewWindow) -> f64 {
    if POINTS { 1.0 } else { w.scale_factor().unwrap_or(1.0) }
}

fn screen_pos(x: i32, y: i32) -> Position {
    if POINTS { LogicalPosition::new(x, y).into() } else { PhysicalPosition::new(x, y).into() }
}

fn screen_size(w: i32, h: i32) -> Size {
    if POINTS { LogicalSize::new(w, h).into() } else { PhysicalSize::new(w as u32, h as u32).into() }
}

fn toggle_popup(app: &AppHandle, from_tray: bool) {
    let Some(w) = popup(app) else { return };
    if w.is_visible().unwrap_or(false) {
        hide_popup(app);
        return;
    }
    let st = app.state::<AppState>();

    let target = if from_tray { 0 } else { platform::foreground() };
    *st.target.lock().unwrap() = target;

    let scale = screen_scale(&w);
    let px = |v: f64| (v * scale).round() as i32;
    let (cx, top, bottom) = platform::caret_or_cursor();
    let work = platform::work_area(cx, top);
    let (margin, gap) = (px(MARGIN), px(GAP));
    let width = px(CARD_W + 2.0 * MARGIN);
    let height = *st.height_px.lock().unwrap();

    let above = bottom + gap + px(MAX_H) > work.bottom;
    let x = (cx - margin).min(work.right - width + margin).max(work.left - margin);
    let placement = Placement { x, caret_top: top, caret_bottom: bottom, work, above, margin, gap };
    let y = y_for(&placement, height);
    *st.placement.lock().unwrap() = Some(placement);

    let _ = w.set_size(screen_size(width, height));
    let _ = w.set_position(screen_pos(x, y));
    *st.shown_at.lock().unwrap() = Instant::now();
    let _ = app.emit("stash-show", ());
    let _ = w.show();
    let _ = w.set_focus();
    platform::focus_popup(&w);
}

fn y_for(p: &Placement, height: i32) -> i32 {
    let y = if p.above { p.caret_top - p.gap - height + p.margin } else { p.caret_bottom + p.gap - p.margin };
    y.min(p.work.bottom - height + p.margin).max(p.work.top - p.margin)
}

fn hide_popup(app: &AppHandle) {
    if let Some(w) = popup(app) {
        let _ = w.hide();
        platform::release_focus(&w);
    }
}

#[tauri::command]
fn list_clips(state: State<AppState>) -> Result<Vec<Clip>, String> {
    state.db.list().map_err(|e| e.to_string())
}

fn put_on_clipboard(state: &AppState, id: i64) -> Result<(), String> {
    let p = state.db.payload(id).map_err(|e| e.to_string())?.ok_or("clip no longer exists")?;
    state.quiet_until.store(now_ms() + ECHO_MS, Ordering::Relaxed);
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    capture::write(&mut cb, &p.kind, &p.text, p.data.as_deref())?;
    state.db.mark_used(id, now_ms()).map_err(|e| e.to_string())
}

#[tauri::command]
fn paste_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    stop_running_stack(&app);
    put_on_clipboard(&state, id)?;
    hide_popup(&app);
    let target = *state.target.lock().unwrap();
    thread::spawn(move || platform::paste_into(target));
    Ok(())
}

fn stack_status(app: &AppHandle) -> StackStatus {
    let st = app.state::<AppState>();
    let s = st.stack.lock().unwrap();
    StackStatus { ids: s.queue.iter().copied().collect(), done: s.done }
}

fn emit_stack(app: &AppHandle) {
    let _ = app.emit("stash-stack", stack_status(app));
}

fn end_stack(app: &AppHandle) {
    *app.state::<AppState>().stack.lock().unwrap() = Stack::default();
    emit_stack(app);
}

fn stop_running_stack(app: &AppHandle) {
    if !app.state::<AppState>().stack.lock().unwrap().queue.is_empty() {
        end_stack(app);
    }
}

fn load_front(st: &AppState, s: &mut Stack) {
    while let Some(&id) = s.queue.front() {
        if put_on_clipboard(st, id).is_ok() {
            if s.watch.is_none() {
                s.watch = Some(platform::watch_pastes(st.pasted.clone()));
            }
            return;
        }
        s.queue.pop_front();
    }
    *s = Stack::default();
}

fn advance_stack(app: &AppHandle) {
    let st = app.state::<AppState>();
    {
        let mut s = st.stack.lock().unwrap();
        if s.queue.pop_front().is_none() {
            return;
        }
        s.done += 1;
        load_front(&st, &mut s);
    }
    emit_stack(app);
}

fn toggle_in_stack(app: &AppHandle, id: i64) {
    let state = app.state::<AppState>();
    {
        let mut s = state.stack.lock().unwrap();
        match s.queue.iter().position(|&x| x == id) {
            Some(0) => {
                s.queue.pop_front();
                load_front(&state, &mut s);
            }
            Some(i) => {
                s.queue.remove(i);
            }
            None => {
                s.queue.push_back(id);
                if s.queue.len() == 1 {
                    load_front(&state, &mut s);
                }
            }
        }
    }
    emit_stack(app);
}

#[tauri::command]
fn toggle_stack(app: AppHandle, id: i64) -> StackStatus {
    toggle_in_stack(&app, id);
    stack_status(&app)
}

#[tauri::command]
fn paste_stack(app: AppHandle, state: State<AppState>) {
    hide_popup(&app);
    let target = *state.target.lock().unwrap();
    if target == 0 {
        return;
    }
    thread::spawn(move || {
        platform::paste_into(target);
        thread::sleep(Duration::from_millis(150));
        advance_stack(&app);
    });
}

#[tauri::command]
fn cancel_stack(app: AppHandle) {
    end_stack(&app);
}

#[tauri::command]
fn get_stack(app: AppHandle) -> StackStatus {
    stack_status(&app)
}

#[tauri::command]
fn copy_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    stop_running_stack(&app);
    put_on_clipboard(&state, id)?;
    hide_popup(&app);
    Ok(())
}

#[tauri::command]
fn toggle_pin(state: State<AppState>, id: i64) -> Result<(), String> {
    state.db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    if state.stack.lock().unwrap().queue.contains(&id) {
        toggle_in_stack(&app, id);
    }
    state.db.delete(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn visual_source(state: State<AppState>, id: i64) -> Result<tauri::ipc::Response, String> {
    let p = state.db.payload(id).map_err(|e| e.to_string())?.ok_or("clip no longer exists")?;
    capture::visual_source(&p.kind, &p.text).map(tauri::ipc::Response::new).ok_or_else(|| "nothing to draw".into())
}

#[tauri::command]
fn save_thumb(state: State<AppState>, id: i64, thumb: String, width: u32, height: u32) -> Result<(), String> {
    let bytes = capture::thumb_from_data_url(&thumb).ok_or("not a PNG or JPEG thumbnail")?;
    state.db.set_thumb(id, &bytes, width, height).map(|_| ()).map_err(|e| e.to_string())
}

fn clip_path(state: &AppState, id: i64) -> Result<(String, String), String> {
    let p = state.db.payload(id).map_err(|e| e.to_string())?.ok_or("clip no longer exists")?;
    let first = p.text.lines().next().unwrap_or("").trim().to_string();
    Ok((p.kind, first))
}

#[tauri::command]
fn reveal_in_explorer(state: State<AppState>, id: i64) -> Result<(), String> {
    let (kind, path) = clip_path(&state, id)?;
    if kind != "file" && kind != "path" {
        return Err("this clip is not a file".into());
    }
    if path.is_empty() {
        return Err("this clip has no path".into());
    }
    platform::reveal(&path);
    Ok(())
}

#[tauri::command]
fn open_url(state: State<AppState>, id: i64) -> Result<(), String> {
    let (kind, url) = clip_path(&state, id)?;
    if kind != "link" {
        return Err("this clip is not a link".into());
    }
    let url = if url.contains("://") { url } else { format!("https://{url}") };
    platform::open(&url);
    Ok(())
}

fn fence(text: &str, lang: &str) -> String {
    let body = text.trim_end_matches(['\r', '\n']);
    let mut longest = 0;
    let mut run = 0;
    for ch in body.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let ticks = "`".repeat((longest + 1).max(3));
    format!("{ticks}{lang}\n{body}\n{ticks}")
}

#[tauri::command]
fn copy_text(state: State<AppState>, id: Option<i64>, text: String) -> Result<(), String> {
    state.quiet_until.store(now_ms() + ECHO_MS, Ordering::Relaxed);
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())?;
    if let Some(id) = id {
        state.db.mark_used(id, now_ms()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn copy_fence(state: State<AppState>, id: i64, lang: String) -> Result<(), String> {
    let p = state.db.payload(id).map_err(|e| e.to_string())?.ok_or("clip no longer exists")?;
    if p.kind != "code" {
        return Err("this clip is not code".into());
    }
    let lang: String = lang
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '-' | '_'))
        .take(16)
        .collect();
    let body = fence(&p.text, &lang);
    state.quiet_until.store(now_ms() + ECHO_MS, Ordering::Relaxed);
    arboard::Clipboard::new().and_then(|mut c| c.set_text(body)).map_err(|e| e.to_string())?;
    state.db.mark_used(id, now_ms()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    hide_popup(&app);
}

#[tauri::command]
fn fit_window(app: AppHandle, state: State<AppState>, height: f64) {
    let Some(w) = popup(&app) else { return };
    let scale = screen_scale(&w);
    let h = height.clamp(MIN_H, MAX_H) + 2.0 * MARGIN;
    let height_px = (h * scale).round() as i32;
    *state.height_px.lock().unwrap() = height_px;
    let _ = w.set_size(LogicalSize::new(CARD_W + 2.0 * MARGIN, h));
    if let Some(p) = state.placement.lock().unwrap().as_ref() {
        let _ = w.set_position(screen_pos(p.x, y_for(p, height_px)));
    }
}

struct Watcher {
    app: AppHandle,
    db: Arc<Db>,
    paused: Arc<AtomicBool>,
    quiet_until: Arc<AtomicI64>,
    cfg: Arc<RwLock<config::Config>>,
}

impl Watcher {
    fn capture(&self) {
        if now_ms() < self.quiet_until.load(Ordering::Relaxed) {
            return;
        }
        stop_running_stack(&self.app);
        let cfg = self.cfg.read().unwrap_or_else(|e| e.into_inner()).clone();
        if cfg.ignore_sensitive && platform::clipboard_is_sensitive() {
            return;
        }
        let limits = capture::Limits {
            max_chars: cfg.max_clip_chars,
            max_image_pixels: cfg.max_image_megapixels as u64 * 1_000_000,
            images: cfg.capture_images,
            files: cfg.capture_files,
            ocr: cfg.ocr_images && cfg.capture_images,
        };
        for _ in 0..6 {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Some(entry) = capture::read(&mut cb, &limits) {
                    if let Err(e) = self.db.add(&entry, now_ms(), cfg.max_items) {
                        eprintln!("stash: could not save clip: {e}");
                    }
                }
                return;
            }
            thread::sleep(Duration::from_millis(40));
        }
    }
}

impl ClipboardHandler for Watcher {
    // macOS has no change notification, so the clipboard is polled; stay well inside ECHO_MS.
    fn sleep_interval(&self) -> Duration {
        Duration::from_millis(200)
    }

    fn on_clipboard_change(&mut self) -> CallbackResult {
        if !self.paused.load(Ordering::Relaxed) {
            self.capture();
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
        eprintln!("stash: clipboard error: {error}");
        CallbackResult::Next
    }
}

fn bind_hotkey(app: &AppHandle, hotkey: &str) -> bool {
    let bound = app.global_shortcut().on_shortcut(hotkey, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_popup(app, false);
        }
    });
    if let Err(e) = &bound {
        eprintln!("stash: could not register hotkey {hotkey:?}: {e} (use the tray icon instead)");
    }
    bound.is_ok()
}

fn apply_config(app: &AppHandle, live: &RwLock<config::Config>, mut new: config::Config) {
    let old = live.read().unwrap_or_else(|e| e.into_inner()).clone();
    if new == old {
        return;
    }
    if new.hotkey != old.hotkey {
        let _ = app.global_shortcut().unregister(old.hotkey.as_str());
        if !bind_hotkey(app, &new.hotkey) {
            bind_hotkey(app, &old.hotkey);
            new.hotkey = old.hotkey;
        }
    }
    *live.write().unwrap_or_else(|e| e.into_inner()) = new;
    eprintln!("stash: config reloaded");
}

fn watch_config(app: AppHandle, mut watch: config::Watch, live: Arc<RwLock<config::Config>>) {
    thread::spawn(move || loop {
        thread::sleep(CONFIG_POLL);
        if let Some(new) = watch.poll() {
            apply_config(&app, &live, new);
        }
    });
}

fn build_tray(app: &tauri::App, db: Arc<Db>, paused: Arc<AtomicBool>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause capturing", true, false, None::<&str>)?;
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_label = if cfg!(target_os = "macos") { "Open at login" } else { "Start with Windows" };
    let autostart = CheckMenuItem::with_id(app, "autostart", autostart_label, true, autostart_on, None::<&str>)?;
    let clear = MenuItem::with_id(app, "clear", "Clear history (keeps pinned)", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &autostart, &clear, &sep, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .tooltip("stash")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => toggle_popup(app, true),
            "pause" => paused.store(pause.is_checked().unwrap_or(false), Ordering::Relaxed),
            "autostart" => {
                let mgr = app.autolaunch();
                let _ = if autostart.is_checked().unwrap_or(false) { mgr.enable() } else { mgr.disable() };
            }
            "clear" => {
                let _ = db.clear_unpinned();
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_popup(tray.app_handle(), true);
            }
        });
    if cfg!(target_os = "macos") {
        // A template image is tinted by macOS to match the menu bar.
        tray = tray.icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?).icon_as_template(true);
    } else if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // No app menu beyond Edit, so Cmd+C/V/X/A/Z work in the search field and Cmd+Q/W can't close stash.
    #[cfg(target_os = "macos")]
    let builder = builder.menu(|app| {
        use tauri::menu::Submenu;
        let edit = Submenu::with_items(
            app,
            "Edit",
            true,
            &[
                &PredefinedMenuItem::undo(app, None)?,
                &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ],
        )?;
        Menu::with_items(app, &[&edit])
    });
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || toggle_popup(&handle, false));
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            list_clips, paste_clip, copy_clip, toggle_pin, delete_clip, hide_window, fit_window, toggle_stack,
            paste_stack, cancel_stack, get_stack, reveal_in_explorer, open_url, copy_text, copy_fence, visual_source,
            save_thumb
        ])
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => {
                let st = window.state::<AppState>();
                if st.shown_at.lock().unwrap().elapsed() > BLUR_GRACE {
                    let _ = window.hide();
                }
            }
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                hide_popup(window.app_handle());
            }
            _ => {}
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if let Some(w) = popup(app.handle()) {
                platform::prepare_popup(&w);
            }
            platform::request_access();
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let config_watch = config::Watch::new(&dir);
            let cfg = Arc::new(RwLock::new(config::load(&dir)));
            let db = Arc::new(Db::open(&dir.join("clips.db"))?);
            let paused = Arc::new(AtomicBool::new(false));
            let quiet_until = Arc::new(AtomicI64::new(0));
            let (pasted, pastes) = channel::<()>();

            app.manage(AppState {
                stack: Mutex::new(Stack::default()),
                pasted,
                db: db.clone(),
                quiet_until: quiet_until.clone(),
                target: Mutex::new(0),
                placement: Mutex::new(None),
                height_px: Mutex::new(360),
                shown_at: Mutex::new(Instant::now()),
            });

            let hotkey = cfg.read().unwrap_or_else(|e| e.into_inner()).hotkey.clone();
            bind_hotkey(app.handle(), &hotkey);
            watch_config(app.handle().clone(), config_watch, cfg.clone());

            let first_run = dir.join(".autostart-init");
            if !first_run.exists() && !cfg!(debug_assertions) {
                let _ = app.autolaunch().enable();
                let _ = std::fs::write(&first_run, b"");
            }

            build_tray(app, db.clone(), paused.clone())?;

            let handle = app.handle().clone();
            thread::spawn(move || {
                for () in pastes {
                    thread::sleep(Duration::from_millis(120));
                    advance_stack(&handle);
                }
            });

            // Screenshots saved as files go on the clipboard, so they are kept and ready to paste.
            let (live, off) = (cfg.clone(), paused.clone());
            platform::watch_screenshots(move |path| {
                let on = live.read().unwrap_or_else(|e| e.into_inner()).capture_screenshots;
                if on && !off.load(Ordering::Relaxed) && !platform::copy_image_file(path) {
                    eprintln!("stash: could not read screenshot {}", path.display());
                }
            });

            let handle = app.handle().clone();
            thread::spawn(move || {
                let watcher = Watcher { app: handle, db, paused, quiet_until, cfg };
                match Master::new(watcher) {
                    Ok(mut master) => {
                        if let Err(e) = master.run() {
                            eprintln!("stash: clipboard watcher stopped: {e}");
                        }
                    }
                    Err(e) => eprintln!("stash: could not start clipboard watcher: {e}"),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running stash");
}

#[cfg(test)]
mod tests {
    use super::fence;

    #[test]
    fn fences_code_and_outgrows_backticks() {
        assert_eq!(fence("let x = 1;\n", "rust"), "```rust\nlet x = 1;\n```");
        assert_eq!(fence("a ``` b", ""), "````\na ``` b\n````");
        assert_eq!(fence("plain", ""), "```\nplain\n```");
    }
}
