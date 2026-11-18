# Krople test app prompt

Build a minimal Tauri v2 test app for Windows 11 to check a "floating icon + main window" pattern. Use placeholder content only, no real app features yet. The goal is to confirm the window behavior works before building the real app.

Background: Krople will be a small helper that runs next to DrEryk (the doctor's main program) at an eye doctor's practice. After putting drops in a patient's eyes, the doctor clicks the floating icon, adds the patient, and minimizes Krople. Timers run, and when one runs out the floating icon turns from white to red. This test app only checks the window pattern and the timer mechanics with placeholder content.

Project location: create the new Tauri app in its own folder, `krople-test/`. The repo already holds an unrelated browser experiment (index.html, app.js, sw.js, style.css, manifest.json, serve.py and others). The Tauri app must not use or modify any of those files.

Language: all text the user sees in the app (buttons, labels, tooltips, placeholder text) is in Polish. Use the Polish strings given below exactly. Code, comments and the README are in English.

## Tech

- Tauri v2 with a plain HTML, CSS and vanilla JavaScript (or TypeScript) frontend. No frontend framework.
- Keep the Rust side minimal and well commented. I'm a web developer and new to Rust and Tauri.
- Tell me how to run it in dev mode and how to build a Windows installer locally.
- WebView2 is built into Windows 11, so nothing extra needs to be installed. Say so in the README.
- No auto-update. Updates are installed by hand by reinstalling, because an updater needs HTTP. Say so in the README.
- The app is unsigned, so SmartScreen will warn. Code signing is a later decision. Say so in the README.

## Windows

The app opens two windows at startup, both defined in tauri.conf.json:

1. Icon window ("icon")
   - About 72×72 px, frameless (no title bar), always on top, not shown in the taskbar, not resizable.
   - Created with `focus: false`, so it never takes keyboard focus when it appears.
   - Uses `decorations: false`, `transparent: true` and `shadow: false`. Windows 11 adds a shadow and a thin border to undecorated windows, which would show a square frame around the round icon.
   - Transparent background with a round colored circle in the middle, showing a number.
   - The circle is white when no timer has run out, and red when at least one timer has run out. This is the same whether the main window is minimized or not.
   - The white circle must have a dark outline or shadow, so it stays visible over DrEryk's white screens.
   - The number is the ready count: how many timers have run out.
   - Turning red only changes the icon's CSS. It must never call `set_focus` or `show`, and it must not move or activate any window. This way, keystrokes the doctor types into DrEryk keep going to DrEryk.
   - Can be dragged anywhere on screen (use data-tauri-drag-region, but make sure a click without dragging still counts as a click).
   - Clicking it shows the main window, restores it if minimized, maximizes it, and brings it to focus.
   - Its position is remembered between restarts (window-state plugin). The plugin tracks only the icon window, and only its position (for example `StateFlags::POSITION`, with the main window excluded).
   - At startup, if the saved position is off-screen (e.g. an external monitor was unplugged), move it to a visible spot in the bottom-right corner of the primary monitor.

2. Main window ("main")
   - A normal window with a title bar, title "Krople".
   - Top bar: at the top right, a minimize icon button (tooltip "Minimalizuj") and a gear icon button (tooltip "Ustawienia"). Next to the minimize button there is a "Zakończ" (Quit) button. The top bar has no DrEryk wording.
   - The minimize icon button minimizes the main window.
   - The gear button opens the settings panel (see "Settings placeholder" below).
   - Placeholder content: the text "Tu będzie pełna aplikacja", and the timer test controls (below).
   - Closing the main window with X should hide it instead of quitting the app, so the icon keeps working. Quitting happens only through the "Zakończ" button in the main window.

## Timer test and shared state

The placeholder app tests timer mechanics: several countdowns running at once.

- Controls in the main window: a button "Dodaj timer 30 s", a button "Dodaj timer 2 min", and a button "Wymuś koniec" (expire now). "Wymuś koniec" makes all running timers run out immediately, to save waiting time while testing.
- Below the buttons, a plain list of timers. Each row shows the timer's label, its remaining time (e.g. "0:18") or, once it has run out, the overdue time with a plus sign (e.g. "+0:12"), and a "Usuń" (delete) button that removes that timer.
- Timer labels are generated automatically ("Timer 1", "Timer 2", ...). There is no label text field. Labels are kept only in memory.
- Each timer stores only its end time, as a timestamp. Remaining or overdue time is always calculated from that timestamp, never by counting ticks.

Timers live in Rust:
- Windows throttles JavaScript in minimized windows, so an alert driven by the main window's JavaScript can fire late. Therefore Rust holds the state: a `Vec<{id, label, ends_at}>`.
- Rust checks it about once a second and sends one `state` event with the payload `{timers, ready_count}` to both windows. Both windows only render what they receive. Add a comment in the code that explains this choice (Rust state, not JavaScript timers or events between windows) and why.
- Rust compares against wall-clock end times (`SystemTime`), not a monotonic clock that may pause in sleep. So a timer that ran out during sleep shows red right after the laptop wakes.
- Each window also asks Rust for the current state on startup, so it does not miss the first event.

Consistency log:
- Every 10 seconds, log to the terminal running `tauri dev`: Rust's ready count versus what each window currently shows, and for each expired timer, its expected turn-red time versus the actual turn-red time. Each window reports what it shows to Rust through a command, and Rust prints the comparison, because each webview has its own devtools and no single console sees both windows. I will use this to check behavior while the main window is minimized and after the laptop wakes from sleep.

## Settings placeholder

- The gear button opens a simple settings panel in the main window with:
  - one number field "Czas oczekiwania, min", default 15, and
  - a checkbox "Uruchamiaj z Windows" (autostart, see Plugins).
- The number is saved with `tauri-plugin-store` and reloaded when the app starts.
- This saved value is only saved and reloaded in this test app. It does not drive the 30 s and 2 min test timers, which are fixed. The point is to test the one disk write the real app will need.

## Plugins

- single-instance: starting the app a second time just shows the main window.
- autostart: the checkbox "Uruchamiaj z Windows" in the settings panel enables or disables starting with Windows.
- window-state: remembers only the icon window position (main window excluded).
- store (`tauri-plugin-store`): saves the "Czas oczekiwania, min" setting.

## Security (important: this will run on a doctor's computer with patient data)

- Capabilities: allow only what's needed for the above (window show, hide, minimize, maximize, unminimize, set focus, start dragging, events, and the four plugins: single-instance, autostart, window-state, store). The plugin permissions are for autostart, window-state and store; single-instance is Rust-only (no capability entry). Grant store permissions only to the main window, and don't add the fs plugin. No HTTP, no shell, no file-system plugin (the frontend cannot read or write arbitrary files), no other plugins.
- A strict Content Security Policy: no remote resources at all, only the app's own files. For example `default-src 'self'; connect-src ipc: http://ipc.localhost`, so Tauri IPC works.
- Installer: NSIS, per-user install (installMode "currentUser"), so no admin rights are needed.
- Patient data: patient labels (initials or first name only) are kept only in memory and are gone when the app quits. The only files the app itself writes are the settings file (store plugin) and the icon position file (window-state plugin). WebView2 also keeps its own profile folder (cache and local storage, under %LOCALAPPDATA%\<identifier>\EBWebView), and autostart adds one registry entry under HKCU\...\Run. None of these hold patient labels.
- Explain in the README what the app can and cannot access, in plain language. Include the points above: the list of allowed plugins (including store), that labels exist only in memory, which two files the app writes, and the WebView2 profile folder and the autostart registry entry.

## Test checklist

Add a README section with a checklist I can go through on Windows 11:
1. The icon stays on top of a full-screen app (e.g. a maximized window), and on top of DrEryk itself, not only a generic maximized window
2. Clicking the icon restores and maximizes the main window from minimized
3. The minimize icon button minimizes the main window and the icon stays visible
4. Dragging the icon works and the position is remembered after restart
5. The icon looks right at 100%, 125% and 150% display scaling
6. Closing the main window with X keeps the app running
7. A second launch doesn't start a second copy
8. Autostart works after a reboot
9. Timers stay correct while the main window is minimized and after sleep
10. The installer runs without admin rights, and note what Windows SmartScreen shows
11. A timer that runs out while the main window is minimized turns the icon red within about 1 second
12. Two timers running out close together are both counted (the icon number goes up by two)
13. Type in DrEryk while a timer runs out: the keystrokes still go to DrEryk, and the icon turning red does not take focus
14. The white icon is visible over a white background
15. The "Czas oczekiwania, min" setting survives a restart, and the timers don't
