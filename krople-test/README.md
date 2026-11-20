# Krople

A Tauri v2 app for Windows 11: a timer helper for an eye doctor's office. It tracks patients who are getting eye drops, shows one card per patient, and turns a floating icon red when someone needs attention. The folder and installer are still called `krople-test` / "Krople Test" (renaming would move the settings folder; a separate decision).

## What it does

- **Icon window**: a small round icon, always on top, with no taskbar entry. It is white while no card is red and red while at least one is, and the number on it is the count of red cards (also while the main window is open). Click it to open the main window (shown, restored, maximized and focused). Drag it to move it, and it remembers its position.
- **Main window**: a top row with the "Inicjały lub imię" field, the "1 kropla" and "3 krople co N min" buttons (N follows the interval setting), a minimize button and a settings (gear) button. Below it is a grid of patient cards in arrival order (new ones on the right; the grid wraps and the window scrolls). An empty list shows "Brak pacjentów". Adding a patient needs a non-empty name (trimmed, up to 20 characters); the window stays open and the field keeps focus (it is not re-focused while a card button is being held).
- **Cards** ("oko z postępem" style): name and protocol, a pie showing progress with the time ("m:ss" while waiting, "+N min" when overdue), drop dots, a status line ("Czeka", "Kropić teraz", "Gotowy do badania") and one action button. A card is red when a drop is due or the patient is ready for the exam. The grid uses `auto-fill`, so a few cards don't stretch across the screen. The "Kropić teraz" state still shows the dots given so far.
- **Press and hold**: "Zakropione", "Zbadany", the "×" (top-right corner of the card, removes the patient) and "Zakończ" must be held for about 1.5 s while the button fills; a plain click does nothing. Releasing early or leaving the button cancels. Space/Enter held on a focused button works the same, and so does a finger on a touch screen (no scrolling or context menu on these buttons). After a card action the name field gets the focus back. "Zakropione" and "Zbadany" also work early, and the next interval starts from that moment. There is no undo: for a mistake, remove the card and add the patient again.
- **Closing**: X / Alt+F4 on the main window only hides it; the icon reopens it with the patients intact. The only way to quit is holding "Zakończ" on the settings page (quitting loses all patients). If the window was hidden with the settings open, reopening it from the icon goes back to the cards.
- **Settings** (gear, shown in place of the grid; "Wróć" goes back): "Odstęp między kroplami, min" (default 10, 1-60), "Czas oczekiwania, min" (default 15, 1-120), "Uruchamiaj z Windows" and "Zakończ". Changes apply to timers started after the change; running phases keep their durations. Settings are saved to disk; a stored value outside its range is ignored and the default is used.
- **Autostart** starts the app with the `--autostart` argument, and then only the icon appears; the main window opens when the icon is clicked. A manual launch shows the main window. The main window is created hidden and shown by `setup`, so there is no flash. An autostart entry created before this argument existed must be re-toggled once (off, then on).

## Project layout

```
krople-test/
  package.json            Tauri CLI (dev dependency); `npm test` runs `node --test src/`
  app-icon.svg            Source for the app icon (`npx tauri icon app-icon.svg`)
  src/                    Frontend: plain HTML, CSS and JavaScript, no framework, no bundler
    index.html, main.js, main.css   main window (rendering, settings view)
    card.js                 pure card view model (status, pie, dots, button) from a patient
    hold.js                 press-and-hold helper (1.5 s)
    format.js               "m:ss" and "+N min" formatting
    state.js                drops "state" payloads older than the one drawn (rev/now)
    *.test.js               unit tests for card, hold, format and state
    icon.html, icon.js, icon.css   floating icon window
    fonts/                  Inter (latin + latin-ext woff2, OFL), bundled because the CSP forbids remote fonts
  src-tauri/
    tauri.conf.json       Both windows (main starts hidden), CSP, installer settings
    capabilities/         What each window is allowed to call (see Security)
    build.rs              Generates one permission per Rust command
    src/lib.rs            Plugins, window behavior, settings and window commands
    src/timers.rs         Patients, the state machine, the 1-second ticker (with `cargo test` unit tests)
```

## Prerequisites

- **Node.js** (20 or newer) and npm.
- **Rust**: install with [rustup](https://rustup.rs) (`winget install Rustlang.Rustup`).
- **Microsoft C++ Build Tools**: Visual Studio 2022 Build Tools with the "Desktop development with C++" workload (`winget install Microsoft.VisualStudio.2022.BuildTools` and pick that workload).
- **WebView2** is built into Windows 11, so there is nothing extra to install, either for development or on the doctor's computer.

## Run in dev mode

```
cd krople-test
npm install
npm run dev
```

The first build compiles all the Rust dependencies and takes a few minutes. Later builds take seconds. Editing files in `src/` takes effect when you reload the window (Ctrl+R in a window with its devtools open). Editing Rust files rebuilds and restarts the app automatically.

Run the tests with `npm test` (frontend) and `cargo test` in `src-tauri` (state machine).

Right-click is disabled on the icon. To open devtools for a window in dev mode, focus it and press Ctrl+Shift+I.

## Build a Windows installer

```
npm run build
```

The installer ends up at `src-tauri/target/release/bundle/nsis/Krople Test_0.1.0_x64-setup.exe`. It is a per-user install (no admin rights needed) into `%LOCALAPPDATA%\Krople Test`.

- **No auto-update.** To update, build a new installer and install it over the old one. An auto-updater would need network (HTTP) access, which this app deliberately doesn't have.
- **Unsigned.** The installer is not code-signed, so Windows SmartScreen will show "Windows protected your PC". Click "More info", then "Run anyway". Code signing is a later decision.

## How the timers work (and why in Rust)

Windows throttles JavaScript in minimized windows, so a timer in the main window's JavaScript could turn the icon red seconds or minutes late. So Rust holds the only list of patients: `{id, label, protocol (One | Three), drops_given, phase_started_at, phase_ends_at}`. About once a second a Rust thread sends one `state` event to both windows, and the windows only draw what they receive. The payload is `{patients, ready_count, now, rev}`; each patient is sent as `{id, label, protocol, drops_given, phase_started_at, phase_ends_at, ready}` (times in ms since 1970, `ready` = the card is red). Each window also asks for the current state when it starts (`get_state`, same payload), so it never misses the first event.

`rev` goes up by one on every change to the patient list (add, drop, remove). Snapshots are taken under the lock but sent after releasing it, so a tick can arrive after a button's newer broadcast; the windows ignore a payload with a lower `rev` than the one drawn (or the same `rev` with an older `now`), so a removed card never comes back for a second.

Adding a patient counts as the first drop: `drops_given = 1`, and the phase is the interval (3 krople) or the wait (1 kropla). "Zakropione" (`mark_drop`) adds a drop; the next phase is the interval while `drops_given < 3`, then the wait. "Zbadany" and "×" (`remove_patient`) remove the patient. Durations are read from settings when a phase starts. A card is red when `phase_ends_at <= now`. Rust compares against the wall clock, so a phase that ended while the laptop was asleep shows as red right after wake-up. The transitions are pure functions of `now` and the durations, covered by `cargo test`.

## Security: what the app can and cannot access

This app will run on a doctor's computer that holds patient data, so it is locked down.

**The windows (the web part) can only call what they need.** Each window has its own allow-list in `src-tauri/capabilities/`:

- Icon window: receive the state event, read the state (`get_state`), be dragged, ask Rust to show the main window (`show_main`).
- Main window: receive the state event, read the state (`get_state`), `add_patient`, `mark_drop`, `remove_patient`, minimize itself, `quit_app`, and `get_settings`, `set_interval_minutes`, `set_wait_minutes`, `set_autostart`.

Showing, restoring, maximizing and focusing the main window is done by Rust on request, so neither window has direct access to those window functions. The windows cannot call any plugin directly: the settings file and autostart are handled by small Rust commands that do one fixed thing each.

**Plugins (the full list):**

- `single-instance`: a second launch just shows the main window of the running copy.
- `autostart`: turns "start with Windows" on or off.
- `window-state`: remembers the icon window's position (only the icon, only its position).
- `store` (`tauri-plugin-store`): saves the two timing settings.

**No network, no shell, no file access.** There is no HTTP plugin, no shell plugin and no file-system plugin, so the app cannot read or write arbitrary files, run programs or talk to the internet. The Content Security Policy (`default-src 'self'; connect-src ipc: http://ipc.localhost`) blocks loading anything except the app's own files.

**Patient data.** Patient labels (initials or first name only, never surnames) exist only in memory. They are never written to disk and are gone when the app quits.

**What the app writes on the computer:**

1. `%APPDATA%\pl.krople.test\settings.json`: the settings (the interval and the waiting time).
2. `%APPDATA%\pl.krople.test\.window-state.json`: the icon's position.
3. `%LOCALAPPDATA%\pl.krople.test\EBWebView\`: WebView2's own profile folder (cache and similar). The app stores nothing there itself.
4. With autostart on: one registry value under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.

None of these contain patient labels. Rust validates every `add_patient` call (label trimmed, 1-20 characters, protocol must be one of the two values).

**Installer.** NSIS, per-user (`installMode: currentUser`), so no admin rights are needed and nothing is installed for other users.

## Test checklist (Windows 11)

Set both intervals to 1 min in the settings first.

1. [ ] Add "A.K." with "1 kropla" and "B" with "3 krople": cards appear in that order, new ones on the right.
2. [ ] The pie fills while "Czeka"; then "Kropić teraz" / "Gotowy do badania" turn red and show "+N min".
3. [ ] The icon turns red with the right count while the main window is minimized.
4. [ ] A quick click on a card button does nothing; a ~1.5 s hold works.
5. [ ] An early "Zakropione" works and the next interval starts from that moment.
6. [ ] Holding "×" removes the card.
7. [ ] Settings survive a restart, patients don't.
8. [ ] X hides the window and the icon reopens it with patients intact.
9. [ ] A click on "Zakończ" does nothing; holding it ~1.5 s quits the whole app (icon gone, no `krople-test.exe` in Task Manager).
10. [ ] With autostart on, after a reboot only the icon appears.
11. [ ] The window looks right maximized at 1920x1080 and at 125% / 150% scaling; at every scaling the whole card, action button included, is visible without scrolling.
12. [ ] The icon stays on top of other windows, including DrEryk, and doesn't take focus when it turns red.
13. [ ] A second launch doesn't start a second copy.
14. [ ] The installer runs without admin rights. Note what Windows SmartScreen shows: **\_\_**

Extra things worth checking:

- Unplug an external monitor with the icon on it, then restart: the icon should appear in the bottom-right corner of the primary monitor.
- A short click on the icon opens the main window, and a drag moves the icon without opening it.
- Open the settings, close the window with X, click the icon: the cards are shown and the name field has focus.
- On a touch screen: holding a card button or "Zakończ" with a finger works (no scrolling, no context menu).
