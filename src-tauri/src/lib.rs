mod codec;
mod stego;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const PASSWORD_KEY: &str = "fq.encodingPassword";

/// Shared state for the clipboard auto-monitor.
struct MonitorState {
    enabled: AtomicBool,
    last_seen: Mutex<Option<String>>,
    last_written: Mutex<Option<String>>,
}

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

// MARK: - Helpers

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
        .invoke_handler(tauri::generate_handler![
            encode, decode, stego_hide, stego_reveal, qr_svg
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
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
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
