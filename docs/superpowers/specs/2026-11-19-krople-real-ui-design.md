# Krople: the real UI (design + plan)

## Context

The `krople-test` Tauri prototype is done and tested: Rust-owned timers, the always-on-top floating icon that turns red, settings storage, autostart and a locked-down security setup. Its main window is still a test harness (fake 30 s / 2 min timers, "Wymuś koniec", a consistency log). The next step is to turn `krople-test` into the real app by building the main-window UI from the mockup (`Krople – timer gabinetu.html`, variant "Wariant: oko z postępem") and the two real drop protocols. Domain facts are in `docs/krople-real-app-notes.md`.

## What the user decided

- **Build on `krople-test`.** It becomes the real app; no new project.
- **Follow the mockup closely** ("oko z postępem" card style), Polish UI.
- **Adding a patient:** click floating icon → main window opens maximized → type initials/first name into "Inicjały lub imię" → click "1 kropla" or "3 krople co 10 min". The window **stays open** so the doctor can see the new card; they close/minimize it themselves.
- **Order:** cards stay in arrival order; new patients are appended on the right. No reordering, no jumping red cards to the front.
- **Card buttons are press-and-hold** (a plain click does nothing). Applies to "Zakropione", "Zbadany" and the remove "×".
- **Mistakes:** remove the card (hold ×) and add the patient again. No undo.
- **Early drop allowed:** holding "Zakropione" works any time, not only when due; the next interval starts from that moment.
- **X hides, never quits.** Closing the main window (X / Alt+F4) hides it; the floating icon stays and reopens it. The only way to quit is "Zakończ" on the settings page.
- **Autostart shows only the icon.** When launched by Windows autostart, the main window stays hidden until the icon is clicked.
- **Confirmed (previously "unconfirmed" in the notes):** icon is red while the main window is open too; the icon number = count of red cards; "Zakończ" lives on the settings page; timers run in Rust.

## Claude's calls (user can override)

- Hold duration ~1.5 s, with the button visibly filling left→right; releasing early (or pointer leaving the button) cancels and resets. Keyboard: holding Space/Enter on a focused button works the same way.
- "Zbadany" also works early (hold), and removes the card.
- Remove = small "×" in the card's top-right corner, hold-to-confirm, no extra dialog.
- "1 kropla" / "3 krople" are normal clicks (adding is easy to undo). They do nothing when the name field is empty (field gets a brief red outline). Name is trimmed, max ~20 characters. After adding, the field clears and keeps focus. Field is focused whenever the main window is shown.
- Overdue time shows as "+N min" (whole minutes, starting at "+0 min"), as in the mockup; running time shows "m:ss".
- Settings (gear) opens a settings view in place of the card grid with a "Wróć" button: "Odstęp między kroplami, min" (default 10), "Czas oczekiwania, min" (default 15), "Uruchamiaj z Windows", and "Zakończ" (quit). Changes apply to timers started after the change; running timers keep their durations.
- Top bar (mockup row): name field + two add buttons, then on the right: minimize icon button and gear. No "DrEryk"/brand text.
- Many patients: the grid wraps into more rows and the window scrolls vertically. Empty list shows a short muted "Brak pacjentów".
- Icon: unchanged behaviour; red and counting when any card is red (drop due or ready), also while the main window is open.
- Font: bundle Inter (OFL, latin + latin-ext woff2) locally under `src/fonts/`, since the CSP forbids remote fonts; fall back to Segoe UI.
- Test scaffolding goes: test buttons, `expire_all`, `report_shown`, consistency log, `shown_at`/`detected_at` fields, the `chrono` dependency if unused afterwards.
- A manual launch (not autostart) still shows the main window, as now. Autostart is told apart by an argument: `tauri_plugin_autostart::init(..., Some(vec!["--autostart"]))`; in `setup`, if `std::env::args()` contains it, hide `main`. The main window is created hidden (`"visible": false` in `tauri.conf.json`) and `setup` shows + maximizes it for manual launches, so there's no flash. An existing autostart registry entry without the flag must be re-toggled once (not deployed yet, so fine).
- Product name / identifier / installer name stay as-is for now (renaming moves the settings folder; a separate decision).

## Card behaviour (state machine, owned by Rust)

Patient: `{ id, label, protocol: One | Three, drops_given, phase_started_at, phase_ends_at }`. A card is **red** when `phase_ends_at <= now`.

| Protocol / state | Status line | Pie / time | Dots | Button |
|---|---|---|---|---|
| 1 kropla, waiting | "Czeka" | fill = elapsed/wait, "m:ss" | ●  | "Zbadany" (grey) |
| 1 kropla, done | "Gotowy do badania" (red) | full red, "+N min" | ● | "Zbadany" (red) |
| 3 krople, drops 1–2, waiting | "Czeka" | interval, "m:ss" | ●○○ / ●●○ | "Zakropione" (grey) |
| 3 krople, drops 1–2, due | "Kropić teraz" (red) | full red, "+N min" | | "Zakropione" (red) |
| 3 krople, 3 drops, waiting | "Czeka" | wait (15), "m:ss" | ●●● | "Zbadany" (grey) |
| 3 krople, 3 drops, done | "Gotowy do badania" (red) | full, "+N min" | ●●● | "Zbadany" (red) |

Adding a patient = first drop given: `drops_given = 1`, phase = interval (3 krople) or wait (1 kropla). "Zakropione": `drops_given += 1`; new phase = interval if `drops_given < 3`, else wait. "Zbadany" / "×": remove. Durations read from settings at the moment the phase starts.

## Implementation

**Rust — `src-tauri/src/timers.rs`** (rename concept timer → patient, keep the file's ticker/broadcast design and the WHY comments)
- Replace `Timer` with the patient struct above; keep `SystemTime`, `snapshot()`, `broadcast()`, `start_ticker()` (drop the log + icon-moved stays).
- DTO adds `protocol`, `drops_given`, `phase_started_at`, `phase_ends_at`, `ready`; `ready_count` unchanged (icon keeps working).
- Commands: `add_patient(label, protocol)` (validate: trimmed, 1–20 chars, protocol enum), `mark_drop(id)` (only for 3-krople with `drops_given < 3`), `remove_patient(id)`. Remove `add_timer`, `expire_all`, `report_shown`.
- Pure transition functions taking `now` and durations, so they're unit-testable without the clock; add `#[cfg(test)]` tests for every row of the table.

**Rust — `src-tauri/src/lib.rs`**
- Settings: add `interval_minutes` (key `intervalMinutes`, default 10, 1–60) next to `wait_minutes`; `get_settings` returns both; new `set_interval_minutes`. Add a small helper the timer commands use to read current durations from the store.
- Update `generate_handler!` list.

**Permissions** — `src-tauri/build.rs` COMMANDS list and `capabilities/main.json` / `capabilities/icon.json`: swap old commands for `add_patient`, `mark_drop`, `remove_patient`, `set_interval_minutes`; icon keeps only `get_state` + `show_main` (+ drag). Delete stale `permissions/autogenerated/*` files for removed commands (regenerated by build).

**Window behaviour — `lib.rs` `on_window_event`**: `("main", CloseRequested)` → `api.prevent_close()` + `window.hide()` instead of `exit(0)`. `show_main_window` already calls `show()`, so the icon reopens it. `quit_app` stays the only exit.

**Mockup source.** `Krople – timer gabinetu.html` is a nested bundle: its `__bundler/manifest` holds two gzipped+base64 HTML pages, each again a bundle whose `__bundler/template` JSON is the real markup. The chosen screen is page `1bd5d9b7-…` ("Wariant: oko z postępem"). Decode with a short Python script (json + base64 + gzip) to read exact inline styles. Card layout top→bottom: header row (name 34 px/700 left, protocol label 16 px/500 right — put × after it), pie SVG (max 200 px, centered; done state = solid red disc), big time (clamp(64px,6vw,96px)/700, red when overdue), drop dots (24 px circles, filled = given), status line (28 px; red/700 when red, `#4A515B`/600 otherwise), flexible spacer, action button (76 px tall, 26 px; red filled when card is red, white with `#C3C9D1` border otherwise). Card: 4 px border, 16 px radius, 28/24 px padding, 24 px gap. Settings screen has no mockup: build it in the same visual language (same field/button sizes as the add row).

**Frontend — `src/index.html`, `src/main.css`, `src/main.js`**
- Rewrite markup/CSS from the mockup's "oko z postępem" screen (colours `#111418`, `#C62828`/`#B3261E`, `#FDECEC`, `#F4F6F9`, `#DCE1E7`; 72 px add row; card grid `repeat(auto-fit, minmax(260px, 1fr))`; pie SVG with `stroke-dasharray` on r=47 circle, circumference 295.3). Move inline styles from the mockup into classes.
- Render cards with DOM calls + `textContent` (never innerHTML), as now. Keyed update by patient id instead of `replaceChildren` each second, so a button being held isn't destroyed by the 1 s re-render.
- Hold-to-confirm helper (pointerdown/up/leave/cancel + keydown/keyup, CSS fill animation via a custom property; fires once at completion).
- Settings view toggle; reuse the existing `loadSettings` / change-handler pattern.
- `src/format.js`: keep `formatTime` for running time; add overdue "+N min" formatting.
- Focus name field on window focus (`getCurrentWindow().onFocusChanged`; needs no extra permission beyond event listen — verify).

**Icon — `src/icon.js`**: drop `report_shown` call; otherwise unchanged.

**Docs**: update `krople-test/README.md` (what it does, commands, permissions, checklist — remove consistency-log section) and in `docs/krople-real-app-notes.md` move all four "Unconfirmed" items into Facts (all confirmed), and add hold-to-confirm, X-hides and autostart-icon-only.

## Process after approval

1. Create a feature branch from `main`; save this design as `docs/superpowers/specs/2026-11-19-krople-real-ui-design.md` and commit.
2. Write the step-by-step plan (superpowers:writing-plans), then implement Rust state machine test-first, then frontend.

## Verification

- `cargo test` in `src-tauri` — state-machine transitions for every table row, early drop, validation.
- `npm run dev`, set both intervals to 1 min, then manually: add "A.K." 1 kropla and "B" 3 krople; check card order, pie fill, "Czeka" → red "Kropić teraz" / "Gotowy do badania", "+N min", icon turns red with correct count while main window is minimized; quick click on card buttons does nothing; ~1.5 s hold works; early "Zakropione"; × removes; settings survive restart, patients don't; X hides the window and the icon reopens it with patients intact; "Zakończ" quits; with autostart on, after reboot only the icon appears; window at 1920×1080 maximized and at 125%/150% scaling.
- `npm run build` still produces the installer.
