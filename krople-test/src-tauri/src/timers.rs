//! Patient state and the once-a-second ticker.
//!
//! WHY THE PATIENT PHASES LIVE IN RUST (and not in JavaScript):
//!
//! Windows (and WebView2) throttle JavaScript in minimized or hidden windows.
//! `setTimeout`/`setInterval` in a minimized main window can fire seconds or
//! even minutes late, so an alert driven by the main window's JavaScript could
//! turn the icon red late. Sending events from one window to the other has the
//! same problem, because the sending window is the throttled one.
//!
//! So Rust owns the one true list of patients. A plain Rust thread is not
//! throttled. About once a second it works out which patients are due and
//! sends ONE `state` event with `{ patients, ready_count, now }` to BOTH
//! windows. The windows never keep their own timers. They only draw what they
//! receive.
//!
//! Each patient stores only the end time of the current phase
//! (`phase_ends_at`). Remaining or overdue time is always calculated as
//! `phase_ends_at - now`, never by counting ticks, so a late or skipped tick
//! can't make a phase drift.
//!
//! We use `SystemTime` (the wall clock) and not `Instant` (a monotonic clock).
//! On Windows a monotonic clock may not advance while the laptop sleeps. With
//! the wall clock, a phase that ended during sleep is already "ready" on the
//! first tick after wake-up, so the icon turns red right away.
//!
//! Phase durations come from the settings at the moment a phase STARTS and are
//! stored as an end time, so changing a setting never alters a running phase.
//!
//! Patient data note: labels exist ONLY in this in-memory list. They are
//! never written to disk, never logged, and are gone when the app quits.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// Longest allowed patient label, in Unicode characters.
const MAX_LABEL_CHARS: usize = 20;

/// How many drops the 3-drop protocol gives before the final wait.
const THREE_DROPS: u8 = 3;

/// "1 kropla" or "3 krople". Sent to and from the windows as "one"/"three".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    One,
    Three,
}

/// Phase lengths read from the settings when a phase starts.
#[derive(Clone, Copy, Debug)]
pub struct Durations {
    /// Time between drops (3-drop protocol).
    pub interval: Duration,
    /// Time to wait after the last drop before the examination.
    pub wait: Duration,
}

/// One patient, as Rust keeps it.
struct Patient {
    id: u64,
    label: String,
    protocol: Protocol,
    drops_given: u8,
    /// When the current phase began (wall clock).
    phase_started_at: SystemTime,
    /// The moment the current phase ends (wall clock).
    phase_ends_at: SystemTime,
}

impl Patient {
    /// A patient is red (needs attention) once the current phase has ended.
    fn is_red(&self, now: SystemTime) -> bool {
        self.phase_ends_at <= now
    }
}

/// Trims the label and checks it is 1-20 Unicode characters.
fn validate_label(raw: &str) -> Result<String, String> {
    let label = raw.trim();
    let len = label.chars().count();
    if len == 0 || len > MAX_LABEL_CHARS {
        return Err("invalid label".into());
    }
    Ok(label.to_string())
}

/// Adding a patient means the first drop was just given.
fn new_patient(id: u64, label: String, protocol: Protocol, now: SystemTime, d: Durations) -> Patient {
    let phase = match protocol {
        Protocol::Three => d.interval,
        Protocol::One => d.wait,
    };
    Patient { id, label, protocol, drops_given: 1, phase_started_at: now, phase_ends_at: now + phase }
}

/// "Zakropione": the next drop was given (also allowed before it is due).
/// Starts the next phase now: another interval, or the final wait after the
/// third drop.
fn give_drop(p: &mut Patient, now: SystemTime, d: Durations) -> Result<(), String> {
    if p.protocol != Protocol::Three || p.drops_given >= THREE_DROPS {
        return Err("no more drops to give".into());
    }
    p.drops_given += 1;
    let phase = if p.drops_given < THREE_DROPS { d.interval } else { d.wait };
    p.phase_started_at = now;
    p.phase_ends_at = now + phase;
    Ok(())
}

#[derive(Default)]
struct Inner {
    patients: Vec<Patient>,
    next_id: u64,
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

/// One patient, as sent to the windows. Times are milliseconds since 1970
/// (the same format as JavaScript's `Date.now()`).
#[derive(Serialize, Clone)]
pub struct PatientDto {
    id: u64,
    label: String,
    protocol: Protocol,
    drops_given: u8,
    phase_started_at: u64,
    phase_ends_at: u64,
    ready: bool,
}

/// The payload of the `state` event and of the `get_state` command.
#[derive(Serialize, Clone)]
pub struct StatePayload {
    patients: Vec<PatientDto>,
    ready_count: usize,
    /// Rust's "now". Windows compute remaining time as `phase_ends_at - now`,
    /// so both windows and Rust always agree on which patients are due.
    now: u64,
}

fn ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

impl Inner {
    fn snapshot(&self, now: SystemTime) -> StatePayload {
        let patients: Vec<PatientDto> = self
            .patients
            .iter()
            .map(|p| PatientDto {
                id: p.id,
                label: p.label.clone(),
                protocol: p.protocol,
                drops_given: p.drops_given,
                phase_started_at: ms(p.phase_started_at),
                phase_ends_at: ms(p.phase_ends_at),
                ready: p.is_red(now),
            })
            .collect();
        let ready_count = patients.iter().filter(|p| p.ready).count();
        StatePayload { patients, ready_count, now: ms(now) }
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
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let state = app.state::<TimerState>();

        let (payload, icon_moved) = {
            let mut inner = state.0.lock().unwrap();
            let payload = inner.snapshot(SystemTime::now());
            let moved = std::mem::take(&mut inner.icon_moved);
            (payload, moved)
        };
        let _ = app.emit("state", payload);

        if icon_moved {
            crate::save_icon_position(&app);
        }
    });
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

/// Adds a patient whose first drop was just given.
#[tauri::command]
pub fn add_patient(
    app: AppHandle,
    state: tauri::State<TimerState>,
    label: String,
    protocol: Protocol,
) -> Result<(), String> {
    let label = validate_label(&label)?;
    // Read the settings BEFORE taking the lock, so file access never blocks
    // the ticker.
    let durations = crate::current_durations(&app);
    {
        let mut inner = state.0.lock().unwrap();
        inner.next_id += 1;
        let id = inner.next_id;
        inner.patients.push(new_patient(id, label, protocol, SystemTime::now(), durations));
    } // the lock is released here, before broadcast() locks again
    broadcast(&app);
    Ok(())
}

/// "Zakropione": the next drop was given.
#[tauri::command]
pub fn mark_drop(app: AppHandle, state: tauri::State<TimerState>, id: u64) -> Result<(), String> {
    let durations = crate::current_durations(&app);
    {
        let mut inner = state.0.lock().unwrap();
        let patient = inner.patients.iter_mut().find(|p| p.id == id).ok_or("unknown patient")?;
        give_drop(patient, SystemTime::now(), durations)?;
    }
    broadcast(&app);
    Ok(())
}

/// "Zbadany" / "x": removes one patient.
#[tauri::command]
pub fn remove_patient(app: AppHandle, state: tauri::State<TimerState>, id: u64) {
    state.0.lock().unwrap().patients.retain(|p| p.id != id);
    broadcast(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000)
    }
    fn d() -> Durations {
        Durations { interval: Duration::from_secs(600), wait: Duration::from_secs(900) }
    }
    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn one_drop_starts_waiting() {
        let p = new_patient(1, "A.K.".into(), Protocol::One, t0(), d());
        assert_eq!(p.drops_given, 1);
        assert_eq!(p.phase_started_at, t0());
        assert_eq!(p.phase_ends_at, t0() + s(900));
        assert!(!p.is_red(t0() + s(899)));
    }

    #[test]
    fn one_drop_ready_after_wait() {
        let p = new_patient(1, "A.K.".into(), Protocol::One, t0(), d());
        assert!(p.is_red(t0() + s(900)));
        assert!(p.is_red(t0() + s(3600)));
    }

    #[test]
    fn three_drops_first_interval() {
        let p = new_patient(1, "A.K.".into(), Protocol::Three, t0(), d());
        assert_eq!(p.drops_given, 1);
        assert_eq!(p.phase_ends_at, t0() + s(600));
        assert!(!p.is_red(t0() + s(599)));
        assert!(p.is_red(t0() + s(600)));
    }

    #[test]
    fn three_drops_second_drop() {
        let mut p = new_patient(1, "A.K.".into(), Protocol::Three, t0(), d());
        give_drop(&mut p, t0() + s(650), d()).unwrap();
        assert_eq!(p.drops_given, 2);
        assert_eq!(p.phase_started_at, t0() + s(650));
        assert_eq!(p.phase_ends_at, t0() + s(1250));
    }

    #[test]
    fn three_drops_third_drop_starts_wait() {
        let mut p = new_patient(1, "A.K.".into(), Protocol::Three, t0(), d());
        give_drop(&mut p, t0() + s(650), d()).unwrap();
        give_drop(&mut p, t0() + s(1300), d()).unwrap();
        assert_eq!(p.drops_given, 3);
        assert_eq!(p.phase_ends_at, t0() + s(1300 + 900));
        assert!(p.is_red(t0() + s(1300 + 900)));
    }

    #[test]
    fn early_drop_allowed() {
        let mut p = new_patient(1, "A.K.".into(), Protocol::Three, t0(), d());
        assert!(give_drop(&mut p, t0() + s(60), d()).is_ok());
        assert_eq!(p.drops_given, 2);
        assert_eq!(p.phase_ends_at, t0() + s(660));
    }

    #[test]
    fn mark_drop_rejects() {
        let mut one = new_patient(1, "A".into(), Protocol::One, t0(), d());
        assert!(give_drop(&mut one, t0() + s(10), d()).is_err());
        assert_eq!(one.drops_given, 1);
        assert_eq!(one.phase_started_at, t0());
        assert_eq!(one.phase_ends_at, t0() + s(900));

        let mut three = new_patient(2, "B".into(), Protocol::Three, t0(), d());
        give_drop(&mut three, t0() + s(10), d()).unwrap();
        give_drop(&mut three, t0() + s(20), d()).unwrap();
        let ends = three.phase_ends_at;
        assert!(give_drop(&mut three, t0() + s(30), d()).is_err());
        assert_eq!(three.drops_given, 3);
        assert_eq!(three.phase_ends_at, ends);
    }

    #[test]
    fn durations_are_captured_at_phase_start() {
        let other = new_patient(2, "B".into(), Protocol::Three, t0(), d());
        let mut p = new_patient(1, "A".into(), Protocol::Three, t0(), d());
        let changed = Durations { interval: s(120), wait: s(60) };
        give_drop(&mut p, t0() + s(100), changed).unwrap();
        assert_eq!(p.phase_ends_at, t0() + s(100 + 120));
        assert_eq!(other.phase_ends_at, t0() + s(600));
        give_drop(&mut p, t0() + s(200), changed).unwrap();
        assert_eq!(p.phase_ends_at, t0() + s(200 + 60));
    }

    #[test]
    fn is_red_boundary() {
        let p = new_patient(1, "A".into(), Protocol::One, t0(), d());
        assert!(p.is_red(p.phase_ends_at));
        assert!(!p.is_red(p.phase_ends_at - Duration::from_millis(1)));
    }

    #[test]
    fn label_validation() {
        assert_eq!(validate_label("  A.K.  "), Ok("A.K.".to_string()));
        assert!(validate_label("").is_err());
        assert!(validate_label("   ").is_err());
        assert!(validate_label("Żółć").is_ok());
        assert!(validate_label(&"ą".repeat(20)).is_ok());
        assert!(validate_label(&"ą".repeat(21)).is_err());
        assert!(validate_label(&"a".repeat(21)).is_err());
    }

    #[test]
    fn protocol_json() {
        assert_eq!(serde_json::to_string(&Protocol::Three).unwrap(), "\"three\"");
        assert_eq!(serde_json::from_str::<Protocol>("\"one\"").unwrap(), Protocol::One);
        assert!(serde_json::from_str::<Protocol>("\"two\"").is_err());
    }
}
