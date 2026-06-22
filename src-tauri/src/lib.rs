mod codec;
mod stego;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const PASSWORD_KEY: &str = "fq.encodingPassword";
const HOTKEY_KEY: &str = "fq.hotkey";
/// Cross-platform default: ⌘⇧E on macOS, Ctrl+Shift+E on Windows/Linux.
const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+E";

/// Shared state for the clipboard auto-monitor.
struct MonitorState {
    enabled: AtomicBool,
    last_seen: Mutex<Option<String>>,
    last_written: Mutex<Option<String>>,
}

/// Currently-registered global hotkey accelerator string.
struct HotkeyState(Mutex<String>);

// MARK: - Commands (called from the web UI)

#[tauri::command]
fn encode(text: String, key: String) -> String {
    codec::encode(&text, &key)
}

#[tauri::command]
fn decode(text: String, key: String) -> Result<String, String> {
    // Return a stable error code; the frontend localises it.
    codec::decode(&text, &key).map_err(|e| e.code())
}

#[tauri::command]
fn stego_hide(secret: String, cover: String) -> String {
    stego::hide(&secret, &cover)
}

#[tauri::command]
fn stego_reveal(text: String) -> Result<String, String> {
    stego::reveal(&text).map_err(|e| e.code().to_string())
}

/// Render `text` as a scannable QR code, returned as an SVG string.
#[tauri::command]
fn qr_svg(text: String) -> Result<String, String> {
    use qrcode::{render::svg, QrCode};
    let code = QrCode::new(text.as_bytes()).map_err(|_| "qr_too_long".to_string())?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .dark_color(svg::Color("#1c1c1e"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

/// Render `text` as a QR PNG (bytes).
fn qr_png(text: &str) -> Result<Vec<u8>, String> {
    use qrcode::QrCode;
    let code = QrCode::new(text.as_bytes()).map_err(|_| "qr_too_long".to_string())?;
    let luma = code
        .render::<image::Luma<u8>>()
        .min_dimensions(600, 600)
        .quiet_zone(true)
        .build();
    let mut png = Vec::new();
    image::DynamicImage::ImageLuma8(luma)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

/// Save the QR for `text` as a PNG via a native save dialog. Returns the saved
/// path, or `None` if the user cancelled.
#[tauri::command]
fn qr_save(app: tauri::AppHandle, text: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let png = qr_png(&text)?;
    let chosen = app
        .dialog()
        .file()
        .add_filter("PNG image", &["png"])
        .set_file_name("fqencoder-qr.png")
        .blocking_save_file();
    match chosen {
        Some(path) => {
            let pb = path.into_path().map_err(|e| e.to_string())?;
            std::fs::write(&pb, png).map_err(|e| e.to_string())?;
            Ok(Some(pb.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Copy the QR for `text` to the clipboard as an image.
#[tauri::command]
fn qr_copy_image(app: tauri::AppHandle, text: String) -> Result<(), String> {
    use qrcode::QrCode;
    let code = QrCode::new(text.as_bytes()).map_err(|_| "qr_too_long".to_string())?;
    let luma = code
        .render::<image::Luma<u8>>()
        .min_dimensions(600, 600)
        .quiet_zone(true)
        .build();
    let rgba = image::DynamicImage::ImageLuma8(luma).to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let img = tauri::image::Image::new_owned(rgba.into_raw(), w, h);
    app.clipboard().write_image(&img).map_err(|e| e.to_string())
}

/// Register a new global hotkey, replacing the current one. Persists on success.
#[tauri::command]
fn set_hotkey(
    app: tauri::AppHandle,
    accelerator: String,
    state: tauri::State<HotkeyState>,
) -> Result<String, String> {
    let new_sc: Shortcut = accelerator.parse().map_err(|_| "hotkey_invalid".to_string())?;
    let gs = app.global_shortcut();
    let mut current = state.0.lock().unwrap();
    if let Ok(old) = current.parse::<Shortcut>() {
        let _ = gs.unregister(old);
    }
    if gs.register(new_sc).is_err() {
        // Restore the previous binding on failure.
        if let Ok(old) = current.parse::<Shortcut>() {
            let _ = gs.register(old);
        }
        return Err("hotkey_invalid".to_string());
    }
    *current = accelerator.clone();
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(HOTKEY_KEY, accelerator.clone());
        let _ = store.save();
    }
    Ok(accelerator)
}

// MARK: - Helpers

/// Bring the main window to the front.
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Tray menu labels (toggle, show, quit) localised by OS locale: zh* → 正體中文.
fn tray_labels() -> (&'static str, &'static str, &'static str) {
    let zh = sys_locale::get_locale()
        .map(|l| l.to_lowercase().starts_with("zh"))
        .unwrap_or(false);
    if zh {
        ("自動監聽剪貼簿", "打開主視窗", "結束 FQEncoder")
    } else {
        ("Auto-monitor clipboard", "Open window", "Quit FQEncoder")
    }
}

fn read_password(app: &tauri::AppHandle) -> String {
    if let Ok(store) = app.store(STORE_FILE) {
        if let Some(value) = store.get(PASSWORD_KEY) {
            if let Some(s) = value.as_str() {
                return s.to_string();
            }
        }
    }
    String::new()
}

/// Polls the clipboard every 0.5s while enabled and transforms new copies.
/// Loop-safe: the system pasteboard has no cross-platform change counter, so
/// we guard with content comparison instead — skip our own writes
/// (`last_written`) and anything we've already seen (`last_seen`), and never
/// write back an empty or unchanged result.
fn spawn_clipboard_monitor(app: tauri::AppHandle, state: Arc<MonitorState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        if !state.enabled.load(Ordering::Relaxed) {
            continue;
        }
        let text = match app.clipboard().read_text() {
            Ok(t) => t,
            Err(_) => continue, // non-text clipboard
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Layer: ignore content we wrote ourselves.
        if state.last_written.lock().unwrap().as_deref() == Some(text.as_str()) {
            continue;
        }
        // Layer: ignore content we already processed.
        {
            let mut seen = state.last_seen.lock().unwrap();
            if seen.as_deref() == Some(text.as_str()) {
                continue;
            }
            *seen = Some(text.clone());
        }

        let key = read_password(&app);
        let result = if codec::looks_encoded(trimmed, &key) {
            match codec::decode(trimmed, &key) {
                Ok(decoded) => decoded,
                Err(_) => continue,
            }
        } else {
            codec::encode(&text, &key)
        };

        // Layer: never write an unchanged or empty result.
        if result.is_empty() || result == text {
            continue;
        }
        if app.clipboard().write_text(result.clone()).is_ok() {
            *state.last_written.lock().unwrap() = Some(result.clone());
            *state.last_seen.lock().unwrap() = Some(result);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        show_main(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            encode, decode, stego_hide, stego_reveal, qr_svg, qr_save, qr_copy_image, set_hotkey
        ])
        .setup(|app| {
            // Menu-bar resident: no Dock icon on macOS.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let state = Arc::new(MonitorState {
                enabled: AtomicBool::new(false),
                last_seen: Mutex::new(None),
                last_written: Mutex::new(None),
            });
            app.manage(state.clone());

            // Register the global hotkey (stored value, else default).
            let accel = app
                .store(STORE_FILE)
                .ok()
                .and_then(|s| s.get(HOTKEY_KEY))
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| DEFAULT_HOTKEY.to_string());
            if let Ok(sc) = accel.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(sc);
            }
            app.manage(HotkeyState(Mutex::new(accel)));

            // Tray menu.
            let (lbl_toggle, lbl_show, lbl_quit) = tray_labels();
            let toggle = CheckMenuItem::with_id(
                app, "toggle", lbl_toggle, true, false, None::<&str>,
            )?;
            let show = MenuItem::with_id(app, "show", lbl_show, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", lbl_quit, true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&toggle, &PredefinedMenuItem::separator(app)?, &show, &quit],
            )?;

            let toggle_item = toggle.clone();
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("FQEncoder")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => {
                        let state = app.state::<Arc<MonitorState>>();
                        let now = !state.enabled.load(Ordering::Relaxed);
                        state.enabled.store(now, Ordering::Relaxed);
                        let _ = toggle_item.set_checked(now);
                    }
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            spawn_clipboard_monitor(app.handle().clone(), state);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Keep the app resident: hide the window instead of quitting.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
