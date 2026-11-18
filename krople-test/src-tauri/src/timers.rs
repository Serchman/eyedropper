//! Timer state and the once-a-second ticker.
//!
//! WHY THE TIMERS LIVE IN RUST (and not in JavaScript):
//!
//! Windows (and WebView2) throttle JavaScript in minimized or hidden windows.
//! `setTimeout`/`setInterval` in a minimized main window can fire seconds or
//! even minutes late, so an alert driven by the main window's JavaScript could
//! turn the icon red late. Sending events from one window to the other has the
//! same problem, because the sending window is the throttled one.
//!
//! So Rust owns the one true list of timers. A plain Rust thread is not
//! throttled. About once a second it works out which timers have run out and
//! sends ONE `state` event with `{ timers, ready_count, now }` to BOTH windows.
//! The windows never keep their own timers. They only draw what they receive.
//!
//! Each timer stores only its end time (`ends_at`). Remaining or overdue time
//! is always calculated as `ends_at - now`, never by counting ticks, so a late
//! or skipped tick can't make a timer drift.
//!
//! We use `SystemTime` (the wall clock) and not `Instant` (a monotonic clock).
//! On Windows a monotonic clock may not advance while the laptop sleeps. With
//! the wall clock, a timer that ran out during sleep is already "ready" on the
//! first tick after wake-up, so the icon turns red right away.
//!
//! Patient data note: timer labels exist ONLY in this in-memory list. They are
//! never written to disk and are gone when the app quits.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// One timer, as Rust keeps it.
struct Timer {
    id: u64,
    label: String,
    /// The moment the timer runs out (wall clock).
    ends_at: SystemTime,
    /// When the ticker first saw this timer as run out (for the log).
    detected_at: Option<SystemTime>,
    /// Window label ("icon"/"main") -> when that window first reported
    /// showing this timer as run out (for the log).
    shown_at: HashMap<String, SystemTime>,
}

/// What a window last told us it is showing.
struct ShownReport {
    ready_count: usize,
    received_at: SystemTime,
}

#[derive(Default)]
struct Inner {
    timers: Vec<Timer>,
    next_id: u64,
    reports: HashMap<String, ShownReport>,
    /// Set when the icon window moved, so the ticker saves its position.
    icon_moved: bool,
}

/// Shared app state. `Mutex` lets the ticker thread and the commands
/// (which run on other threads) take turns changing it safely.
#[derive(Default)]
pub struct TimerState(Mutex<Inner>);

impl TimerState {
    pub fn mark_icon_moved(&self) {
        self.0.lock().unwrap().icon_moved = true;
    }
}

/// One timer, as sent to the windows. Times are milliseconds since 1970
/// (the same format as JavaScript's `Date.now()`).
#[derive(Serialize, Clone)]
pub struct TimerDto {
    id: u64,
    label: String,
    ends_at: u64,
    ready: bool,
}

/// The payload of the `state` event and of the `get_state` command.
#[derive(Serialize, Clone)]
pub struct StatePayload {
    timers: Vec<TimerDto>,
    ready_count: usize,
    /// Rust's "now". Windows compute remaining time as `ends_at - now`, so
    /// both windows and Rust always agree on which timers have run out.
    now: u64,
}

fn ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

impl Inner {
    /// Builds the payload and records when each timer was first seen as run out.
    fn snapshot(&mut self, now: SystemTime) -> StatePayload {
        let mut ready_count = 0;
        let timers = self
            .timers
            .iter_mut()
            .map(|t| {
                let ready = t.ends_at <= now;
                if ready {
                    ready_count += 1;
                    t.detected_at.get_or_insert(now);
                }
                TimerDto { id: t.id, label: t.label.clone(), ends_at: ms(t.ends_at), ready }
            })
            .collect();
        StatePayload { timers, ready_count, now: ms(now) }
    }
}

/// Sends the current state to both windows right away (used after a button
/// press, so the UI doesn't wait up to a second for the next tick).
fn broadcast(app: &AppHandle) {
    let payload = app.state::<TimerState>().0.lock().unwrap().snapshot(SystemTime::now());
    // `emit` sends to every window. Both windows listen for "state".
    let _ = app.emit("state", payload);
}

/// Starts the background thread that ticks about once a second.
pub fn start_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_log = SystemTime::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let now = SystemTime::now();
            let state = app.state::<TimerState>();

            let (payload, icon_moved) = {
                let mut inner = state.0.lock().unwrap();
                let payload = inner.snapshot(now);
                let moved = std::mem::take(&mut inner.icon_moved);
                (payload, moved)
            };
            let _ = app.emit("state", payload);

            if icon_moved {
                crate::save_icon_position(&app);
            }

            // `duration_since` fails if the clock went backwards; then we just
            // log now and restart the 10 s count.
            if now.duration_since(last_log).map_or(true, |d| d >= Duration::from_secs(10)) {
                last_log = now;
                print_consistency_log(&state.0.lock().unwrap(), now);
            }
        }
    });
}

/// Prints Rust's view versus what each window reports it is showing.
/// Each webview has its own devtools console, so no single console can see
/// both windows. Printing here, in the `tauri dev` terminal, puts everything
/// in one place.
fn print_consistency_log(inner: &Inner, now: SystemTime) {
    let rust_ready = inner.timers.iter().filter(|t| t.ends_at <= now).count();
    let fmt_time = |t: SystemTime| {
        chrono::DateTime::<chrono::Local>::from(t).format("%H:%M:%S%.3f").to_string()
    };
    // Delay of `t` after `base`, as "+0.42 s", or "-0.10 s" if earlier.
    let delay = |t: SystemTime, base: SystemTime| match t.duration_since(base) {
        Ok(d) => format!("+{:.2} s", d.as_secs_f64()),
        Err(e) => format!("-{:.2} s", e.duration().as_secs_f64()),
    };
    let window_report = |label: &str| match inner.reports.get(label) {
        Some(r) => {
            let age = now.duration_since(r.received_at).unwrap_or_default().as_secs_f64();
            let mark = if r.ready_count == rust_ready { "OK" } else { "MISMATCH" };
            format!("{} (reported {:.1} s ago) {}", r.ready_count, age, mark)
        }
        None => "no report yet".to_string(),
    };

    println!("[krople {}] ready count: Rust {} | icon {} | main {}",
        fmt_time(now), rust_ready, window_report("icon"), window_report("main"));

    for t in inner.timers.iter().filter(|t| t.ends_at <= now) {
        let shown = |label: &str| match t.shown_at.get(label) {
            Some(at) => delay(*at, t.ends_at),
            None => "not yet".to_string(),
        };
        println!("    {}: expected red at {} | Rust saw it {} | icon showed it {} | main showed it {}",
            t.label,
            fmt_time(t.ends_at),
            t.detected_at.map_or("not yet".to_string(), |d| delay(d, t.ends_at)),
            shown("icon"),
            shown("main"));
    }
}

// ---------------------------------------------------------------------------
// Commands the windows can call with `invoke(...)`. Which window may call which
// command is set in `capabilities/*.json`.
// ---------------------------------------------------------------------------

/// Returns the current state. Each window calls this on startup so it does
/// not have to wait for (or miss) the first `state` event.
#[tauri::command]
pub fn get_state(state: tauri::State<TimerState>) -> StatePayload {
    state.0.lock().unwrap().snapshot(SystemTime::now())
}

/// A window tells Rust which timers it is currently showing as run out.
/// Used only for the consistency log.
#[tauri::command]
pub fn report_shown(
    window: tauri::WebviewWindow,
    state: tauri::State<TimerState>,
    ready_ids: Vec<u64>,
) {
    let label = window.label().to_string();
    let now = SystemTime::now();
    let mut inner = state.0.lock().unwrap();
    for t in inner.timers.iter_mut().filter(|t| ready_ids.contains(&t.id)) {
        t.shown_at.entry(label.clone()).or_insert(now);
    }
    inner.reports.insert(label, ShownReport { ready_count: ready_ids.len(), received_at: now });
}

/// Adds a timer that runs out `seconds` from now.
#[tauri::command]
pub fn add_timer(app: AppHandle, state: tauri::State<TimerState>, seconds: u64) -> Result<(), String> {
    // The test buttons only send 30 or 120. Reject anything odd anyway.
    if !(1..=24 * 3600).contains(&seconds) {
        return Err("invalid duration".into());
    }
    {
        let mut inner = state.0.lock().unwrap();
        inner.next_id += 1;
        let id = inner.next_id;
        inner.timers.push(Timer {
            id,
            label: format!("Timer {id}"),
            ends_at: SystemTime::now() + Duration::from_secs(seconds),
            detected_at: None,
            shown_at: HashMap::new(),
        });
    } // the lock is released here, before broadcast() locks again
    broadcast(&app);
    Ok(())
}

/// Removes one timer.
#[tauri::command]
pub fn remove_timer(app: AppHandle, state: tauri::State<TimerState>, id: u64) {
    state.0.lock().unwrap().timers.retain(|t| t.id != id);
    broadcast(&app);
}

/// "Wymuś koniec": makes every running timer run out right now.
#[tauri::command]
pub fn expire_all(app: AppHandle, state: tauri::State<TimerState>) {
    {
        let now = SystemTime::now();
        let mut inner = state.0.lock().unwrap();
        for t in inner.timers.iter_mut().filter(|t| t.ends_at > now) {
            t.ends_at = now;
        }
    }
    broadcast(&app);
}
