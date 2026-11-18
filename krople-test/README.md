# Krople test app

A minimal Tauri v2 app for Windows 11 that tests the "floating icon + main window" pattern and the timer mechanics for Krople. All content is placeholder; there are no real app features yet.

## What it does

- **Icon window**: a small round icon, always on top, with no taskbar entry. It is white while no timer has run out and red while at least one has, and the number on it is the count of timers that have run out. Click it to open the main window (shown, restored, maximized and focused). Drag it to move it, and it remembers its position.
- **Main window**: a "Zakończ" (quit) button, a minimize button and a settings (gear) button, plus the timer test controls ("Dodaj timer 30 s", "Dodaj timer 2 min", "Wymuś koniec") and a list of timers. Closing it with X quits the whole app (icon included), the same as "Zakończ".
- **Settings**: "Czas oczekiwania, min" (saved to disk, reloaded at startup; it does not drive the test timers) and "Uruchamiaj z Windows" (autostart).

## Project layout

```
krople-test/
  package.json            Tauri CLI (the only npm dependency, dev only)
  app-icon.svg            Source for the app icon (`npx tauri icon app-icon.svg`)
  src/                    Frontend: plain HTML, CSS and JavaScript, no framework, no bundler
    index.html, main.js, main.css, format.js   main window
    icon.html, icon.js, icon.css               floating icon window
  src-tauri/
    tauri.conf.json       Both windows, CSP, installer settings
    capabilities/         What each window is allowed to call (see Security)
    build.rs              Generates one permission per Rust command
    src/lib.rs            Plugins, window behavior, settings and window commands
    src/timers.rs         Timer state, the 1-second ticker and the consistency log
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

Right-click is disabled on the icon. To open devtools for a window in dev mode, focus it and press Ctrl+Shift+I.

## Build a Windows installer

```
npm run build
```

The installer ends up at `src-tauri/target/release/bundle/nsis/Krople Test_0.1.0_x64-setup.exe`. It is a per-user install (no admin rights needed) into `%LOCALAPPDATA%\Krople Test`.

- **No auto-update.** To update, build a new installer and install it over the old one. An auto-updater would need network (HTTP) access, which this app deliberately doesn't have.
- **Unsigned.** The installer is not code-signed, so Windows SmartScreen will show "Windows protected your PC". Click "More info", then "Run anyway". Code signing is a later decision.

## How the timers work (and why in Rust)

Windows throttles JavaScript in minimized windows, so a timer in the main window's JavaScript could turn the icon red seconds or minutes late. So Rust holds the only list of timers (`{id, label, ends_at}`). About once a second a Rust thread sends one `state` event with `{timers, ready_count, now}` to both windows, and the windows only draw what they receive. Each window also asks for the current state when it starts, so it never misses the first event.

Each timer stores only its end time. Remaining time ("0:18") or overdue time ("+0:12") is always calculated from it. Rust compares against the wall clock, so a timer that ran out while the laptop was asleep shows as run out right after wake-up.

### Consistency log

Every 10 seconds the `npm run dev` terminal prints something like:

```
[krople 14:32:10.004] ready count: Rust 2 | icon 2 (reported 0.3 s ago) OK | main 2 (reported 0.3 s ago) OK
    Timer 1: expected red at 14:31:52.118 | Rust saw it +0.61 s | icon showed it +0.63 s | main showed it +0.65 s
    Timer 2: expected red at 14:32:05.440 | Rust saw it +0.29 s | icon showed it +0.31 s | main showed it +0.34 s
```

- `ready count`: Rust's count against what each window last reported showing. `MISMATCH` means a window is behind. A large "reported ... ago" means that window's JavaScript is being throttled.
- For each expired timer: when it should have turned red, and how long after that Rust and each window showed it as red. Expect under about 1 second. "Wymuś koniec" sets the expected time to the moment you click it.

Each window reports what it shows to Rust after every redraw, because each webview has its own devtools and no single console sees both windows.

## Security: what the app can and cannot access

This app will run on a doctor's computer that holds patient data, so it is locked down.

**The windows (the web part) can only call what they need.** Each window has its own allow-list in `src-tauri/capabilities/`:

- Icon window: receive the state event, report what it shows, be dragged, ask Rust to show the main window.
- Main window: receive the state event, report what it shows, add, remove and expire timers, minimize itself, quit, and read and change its two settings.

Showing, restoring, maximizing and focusing the main window is done by Rust on request, so neither window has direct access to those window functions. The windows cannot call any plugin directly: the settings file and autostart are handled by small Rust commands that do one fixed thing each.

**Plugins (the full list):**

- `single-instance`: a second launch just shows the main window of the running copy.
- `autostart`: turns "start with Windows" on or off.
- `window-state`: remembers the icon window's position (only the icon, only its position).
- `store` (`tauri-plugin-store`): saves the "Czas oczekiwania, min" setting.

**No network, no shell, no file access.** There is no HTTP plugin, no shell plugin and no file-system plugin, so the app cannot read or write arbitrary files, run programs or talk to the internet. The Content Security Policy (`default-src 'self'; connect-src ipc: http://ipc.localhost`) blocks loading anything except the app's own files.

**Patient data.** Timer labels (in the real app: initials or first name only, never surnames) exist only in memory. They are never written to disk and are gone when the app quits.

**What the app writes on the computer:**

1. `%APPDATA%\pl.krople.test\settings.json`: the settings (just the waiting time).
2. `%APPDATA%\pl.krople.test\.window-state.json`: the icon's position.
3. `%LOCALAPPDATA%\pl.krople.test\EBWebView\`: WebView2's own profile folder (cache and similar). The app stores nothing there itself.
4. With autostart on: one registry value under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.

None of these contain patient labels.

**Installer.** NSIS, per-user (`installMode: currentUser`), so no admin rights are needed and nothing is installed for other users.

## Test checklist (Windows 11)

1. [x] The icon stays on top of a full-screen app (e.g. a maximized window), and on top of DrEryk itself, not only a generic maximized window.
2. [x] Clicking the icon restores and maximizes the main window from minimized.
3. [x] The minimize icon button minimizes the main window and the icon stays visible.
4. [x] Dragging the icon works and the position is remembered after restart.
5. [x] The icon looks right at 100%, 125% and 150% display scaling.
6. [x] Closing the main window with X quits the whole app: the icon disappears and no `krople-test.exe` is left in Task Manager. The icon position is still remembered on the next start.
7. [ ] A second launch doesn't start a second copy.
8. [ ] Autostart works after a reboot.
9. [x] Timers stay correct while the main window is minimized and after sleep (check the consistency log).
10. [ ] The installer runs without admin rights. Note what Windows SmartScreen shows: **\_\_**
11. [x] A timer that runs out while the main window is minimized turns the icon red within about 1 second.
12. [x] Two timers running out close together are both counted (the icon number goes up by two).
13. [x] Type in DrEryk while a timer runs out: the keystrokes still go to DrEryk, and the icon turning red does not take focus.
14. [x] The white icon is visible over a white background.
15. [x] The "Czas oczekiwania, min" setting survives a restart, and the timers don't.

Extra things worth checking:

- Unplug an external monitor with the icon on it, then restart: the icon should appear in the bottom-right corner of the primary monitor.
- A short click on the icon opens the main window, and a drag moves the icon without opening it.
