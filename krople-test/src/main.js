// The main window.
//
// Like the icon, it keeps no timers of its own. It only draws the "state"
// that Rust sends about once a second (see src-tauri/src/timers.rs for why).

import { formatTime } from "./format.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (id) => document.getElementById(id);
const list = $("timer-list");
const empty = $("empty");

// ---- Timers --------------------------------------------------------------

function render(state) {
  // Build the rows with DOM calls and textContent (never innerHTML), so a
  // label can never be interpreted as HTML.
  const rows = state.timers.map((t) => {
    const li = document.createElement("li");
    li.classList.toggle("ready", t.ready);

    const label = document.createElement("span");
    label.className = "label";
    label.textContent = t.label;

    const time = document.createElement("span");
    time.className = "time";
    // Always computed from the end time. Using Rust's `now` keeps both
    // windows and Rust in agreement about which timers have run out.
    time.textContent = formatTime(t.ends_at - state.now);

    const del = document.createElement("button");
    del.type = "button";
    del.textContent = "Usuń";
    del.addEventListener("click", () => invoke("remove_timer", { id: t.id }));

    li.append(label, time, del);
    return li;
  });
  list.replaceChildren(...rows);
  empty.hidden = state.timers.length > 0;

  // Tell Rust what we now show, for the consistency log in the terminal.
  const readyIds = state.timers.filter((t) => t.ready).map((t) => t.id);
  invoke("report_shown", { readyIds }).catch(() => {});
}

listen("state", (event) => render(event.payload));
// Ask for the current state right away, so we don't miss the first event.
invoke("get_state").then(render);

$("btn-add-30").addEventListener("click", () => invoke("add_timer", { seconds: 30 }));
$("btn-add-120").addEventListener("click", () => invoke("add_timer", { seconds: 120 }));
$("btn-expire").addEventListener("click", () => invoke("expire_all"));

// ---- Top bar -------------------------------------------------------------

$("btn-minimize").addEventListener("click", () => getCurrentWindow().minimize());
$("btn-quit").addEventListener("click", () => invoke("quit_app"));

const settingsPanel = $("settings");
const settingsBtn = $("btn-settings");
settingsBtn.addEventListener("click", () => {
  settingsPanel.hidden = !settingsPanel.hidden;
  settingsBtn.setAttribute("aria-expanded", String(!settingsPanel.hidden));
  if (!settingsPanel.hidden) loadSettings();
});

// ---- Settings ------------------------------------------------------------

const waitInput = $("wait-minutes");
const autostartBox = $("autostart");
const status = $("settings-status");

async function loadSettings() {
  try {
    const s = await invoke("get_settings");
    waitInput.value = s.wait_minutes;
    autostartBox.checked = s.autostart;
    status.textContent = "";
  } catch (err) {
    status.textContent = "Nie udało się wczytać ustawień";
    console.error(err);
  }
}

waitInput.addEventListener("change", async () => {
  const minutes = Number(waitInput.value);
  if (!Number.isInteger(minutes) || minutes < 1 || minutes > 120) {
    status.textContent = "Podaj liczbę od 1 do 120";
    return;
  }
  try {
    await invoke("set_wait_minutes", { minutes });
    status.textContent = "Zapisano";
  } catch (err) {
    status.textContent = "Nie udało się zapisać";
    console.error(err);
  }
});

autostartBox.addEventListener("change", async () => {
  try {
    await invoke("set_autostart", { enabled: autostartBox.checked });
    status.textContent = "Zapisano";
  } catch (err) {
    autostartBox.checked = !autostartBox.checked;
    status.textContent = "Nie udało się zmienić autostartu";
    console.error(err);
  }
});

// Load once at startup too, so the saved value is in place before the panel opens.
loadSettings();
