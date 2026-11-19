//! Krople test app: the Rust side.
//!
//! Two windows are defined in `tauri.conf.json`:
//!   - "icon": the small round floating icon (always on top, no taskbar entry)
//!   - "main": the normal app window
//!
//! This file wires up the plugins, the window behavior (close = quit, icon
//! position) and the small commands the windows call. The timers themselves
//! are in `timers.rs`.

mod timers;

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartExt};
use tauri_plugin_store::StoreExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};

use timers::{Durations, TimerState};

/// The settings file, saved by the store plugin in the app's data folder.
/// It only ever holds settings, never patient data.
const SETTINGS_FILE: &str = "settings.json";
const WAIT_MINUTES_KEY: &str = "waitMinutes";
const DEFAULT_WAIT_MINUTES: u32 = 15;
const INTERVAL_MINUTES_KEY: &str = "intervalMinutes";
const DEFAULT_INTERVAL_MINUTES: u32 = 10;

/// The window-state plugin remembers only the icon window, and only its
/// position.
const ICON_STATE_FLAGS: StateFlags = StateFlags::POSITION;

pub fn run() {
    tauri::Builder::default()
        // single-instance must be registered first. When the app is started a
        // second time, the second copy exits and this callback runs in the
        // first copy instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        // Autostart adds or removes one registry entry under
        // HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(ICON_STATE_FLAGS)
                .with_denylist(&["main"]) // never remember the main window
                .build(),
        )
        .manage(TimerState::default())
        .invoke_handler(tauri::generate_handler![
            timers::get_state,
            timers::add_patient,
            timers::mark_drop,
            timers::remove_patient,
            show_main,
            quit_app,
            get_settings,
            set_interval_minutes,
            set_wait_minutes,
            set_autostart,
        ])
        .setup(|app| {
            ensure_icon_on_screen(app.handle());
            timers::start_ticker(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // Closing the main window with X quits the whole app, the same as
            // the "Zakończ" button. Without this, the icon window would keep
            // the app running with no main window.
            ("main", WindowEvent::CloseRequested { .. }) => {
                window.app_handle().exit(0);
            }
            // The icon window can't be closed either (e.g. Alt+F4 on it).
            ("icon", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
            }
            // The icon was dragged: the ticker saves its position within a
            // second, so it survives even a power cut or forced shutdown.
            ("icon", WindowEvent::Moved(_)) => {
                window.state::<TimerState>().mark_icon_moved();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running the Krople test app");
}

/// Shows the main window, restores it if minimized, maximizes it and focuses it.
/// Used by the icon click and by a second launch of the app.
fn show_main_window(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.maximize();
        let _ = main.set_focus();
    }
}

/// Saves the icon window position to disk (window-state plugin).
///
/// IMPORTANT: the save must run on the main (UI) thread. The plugin's save
/// locks its cache and then reads the window position. From a background
/// thread, that read has to wait for the main thread. Meanwhile the main
/// thread, handling the icon's "moved" events during a drag, waits for the
/// same cache lock, so both threads wait forever and the app freezes.
/// On the main thread the read happens directly, and a save can't overlap a
/// "moved" event, so there is nothing to wait for.
pub(crate) fn save_icon_position(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = handle.save_window_state(ICON_STATE_FLAGS);
    });
}

/// At startup: restore the saved icon position. If there is none (first run),
/// or the saved spot is no longer on any monitor (e.g. an external monitor
/// was unplugged), put the icon in the bottom-right corner of the primary
/// monitor instead.
fn ensure_icon_on_screen(app: &AppHandle) {
    let Some(icon) = app.get_webview_window("icon") else { return };

    let has_saved_state = app
        .path()
        .app_config_dir()
        .map(|dir| dir.join(app.filename()).exists())
        .unwrap_or(false);
    // The plugin already restores the position when the window is created.
    // Doing it again here makes sure it has happened before we check it.
    let _ = icon.restore_state(ICON_STATE_FLAGS);

    let (Ok(pos), Ok(size)) = (icon.outer_position(), icon.outer_size()) else { return };
    // The icon counts as visible if its center is on some monitor.
    let cx = pos.x + size.width as i32 / 2;
    let cy = pos.y + size.height as i32 / 2;
    let on_screen = icon.available_monitors().unwrap_or_default().iter().any(|m| {
        let (p, s) = (m.position(), m.size());
        cx >= p.x && cx < p.x + s.width as i32 && cy >= p.y && cy < p.y + s.height as i32
    });

    if has_saved_state && on_screen {
        return;
    }
    if let Ok(Some(monitor)) = icon.primary_monitor() {
        // The work area is the monitor minus the taskbar.
        let area = monitor.work_area();
        let margin = (24.0 * monitor.scale_factor()) as i32;
        let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
        let _ = icon.set_position(PhysicalPosition::new(x, y));
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Called when the icon is clicked.
///
/// The window calls this Rust command instead of using the window API itself,
/// so the icon window needs no show/maximize/focus permissions. Note that
/// turning the icon red never calls this: it only changes the icon's CSS, so
/// it can never steal keyboard focus from DrEryk.
#[tauri::command]
fn show_main(app: AppHandle) {
    show_main_window(&app);
}

/// The "Zakończ" button. The only way to quit the app.
#[tauri::command]
fn quit_app(app: AppHandle) {
    // The window-state plugin saves the icon position itself on exit.
    app.exit(0);
}

#[derive(Serialize)]
struct Settings {
    interval_minutes: u32,
    wait_minutes: u32,
    autostart: bool,
}

/// Reads one "minutes" setting, falling back to `default` when the store or
/// the value is missing.
fn read_minutes(app: &AppHandle, key: &str, default: u32) -> u32 {
    app.store(SETTINGS_FILE)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(default)
}

/// The phase lengths a phase starting right now should use.
pub(crate) fn current_durations(app: &AppHandle) -> Durations {
    let minutes = |key, default| Duration::from_secs(read_minutes(app, key, default) as u64 * 60);
    Durations {
        interval: minutes(INTERVAL_MINUTES_KEY, DEFAULT_INTERVAL_MINUTES),
        wait: minutes(WAIT_MINUTES_KEY, DEFAULT_WAIT_MINUTES),
    }
}

/// Loads the settings for the settings panel.
#[tauri::command]
fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let interval_minutes = read_minutes(&app, INTERVAL_MINUTES_KEY, DEFAULT_INTERVAL_MINUTES);
    let wait_minutes = read_minutes(&app, WAIT_MINUTES_KEY, DEFAULT_WAIT_MINUTES);
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    Ok(Settings { interval_minutes, wait_minutes, autostart })
}

/// Saves the time between drops to the settings file. Only phases started
/// after the change use it.
#[tauri::command]
fn set_interval_minutes(app: AppHandle, minutes: u32) -> Result<(), String> {
    if !(1..=60).contains(&minutes) {
        return Err("value out of range".into());
    }
    let store = app.store(SETTINGS_FILE).map_err(|e| e.to_string())?;
    store.set(INTERVAL_MINUTES_KEY, minutes);
    store.save().map_err(|e| e.to_string())
}

/// Saves "Czas oczekiwania, min" to the settings file.
#[tauri::command]
fn set_wait_minutes(app: AppHandle, minutes: u32) -> Result<(), String> {
    if !(1..=120).contains(&minutes) {
        return Err("value out of range".into());
    }
    let store = app.store(SETTINGS_FILE).map_err(|e| e.to_string())?;
    store.set(WAIT_MINUTES_KEY, minutes);
    store.save().map_err(|e| e.to_string())
}

/// The "Uruchamiaj z Windows" checkbox.
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())
}
