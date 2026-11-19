// The main window: the add row and one card per patient.
//
// Like the icon, it keeps no timers of its own. It only draws the "state"
// that Rust sends about once a second (see src-tauri/src/timers.rs for why).

import { cardView, PIE_CIRCUMFERENCE } from "./card.js";
import { holdToConfirm } from "./hold.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (id) => document.getElementById(id);
const nameInput = $("name");
const addOne = $("add-one");
const addThree = $("add-three");
const grid = $("cards");
const empty = $("empty");
const settingsView = $("settings-view");

const SVG_NS = "http://www.w3.org/2000/svg";

// ---- Cards ---------------------------------------------------------------
//
// Keyed by patient id: each card's elements are made once and afterwards
// only updated (text, classes, attributes). Rebuilding them every second
// would destroy a card button while it is being held down.

/** @type {Map<number, object>} patient id -> card refs (see createCard) */
const cards = new Map();

// The last state drawn. A held button reads it when the hold completes, so
// it acts on what the card shows now, not on what it showed at creation.
let latest = null;

function logError(err) {
  console.error(err);
}

// Called once for every new card, after it is in the page. Both card
// buttons only act after a press-and-hold (see hold.js).
function onCardCreated(refs, id) {
  holdToConfirm(refs.action, () => {
    const p = latest?.patients.find((q) => q.id === id);
    if (!p) return;
    // "Zbadany" removes the card; "Zakropione" (also early) marks a drop.
    const command = cardView(p, latest.now).isWait ? "remove_patient" : "mark_drop";
    invoke(command, { id }).catch(logError);
  });
  holdToConfirm(refs.remove, () => invoke("remove_patient", { id }).catch(logError));
}

function el(tag, className, text) {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

function circle(className, attrs) {
  const c = document.createElementNS(SVG_NS, "circle");
  c.setAttribute("class", className);
  c.setAttribute("cx", "100");
  c.setAttribute("cy", "100");
  for (const [k, v] of Object.entries(attrs)) c.setAttribute(k, v);
  return c;
}

// Builds the card's elements, all text through textContent (never innerHTML),
// so a label can never be interpreted as HTML.
function createCard() {
  const article = el("article", "card");

  const header = el("div", "card-header");
  const name = el("h2", "card-name");
  const protocol = el("span", "card-protocol");
  // Placed in the card's top-right corner by CSS.
  const remove = el("button", "card-remove", "×");
  remove.type = "button";
  remove.setAttribute("aria-label", "Usuń");
  header.append(name, protocol, remove);

  // The "eye": white disc, red wedge for elapsed time, black ring and pupil.
  // The wedge is a circle of r=47 with a stroke 94 wide, so its dash length
  // draws a pie slice. When red, CSS fills the disc and hides the wedge.
  const pie = document.createElementNS(SVG_NS, "svg");
  pie.setAttribute("class", "pie");
  pie.setAttribute("viewBox", "0 0 200 200");
  pie.setAttribute("role", "img");
  const disc = circle("pie-disc", { r: "94", fill: "#FFFFFF" });
  const wedge = circle("pie-wedge", {
    r: "47",
    fill: "none",
    stroke: "#C62828",
    "stroke-width": "94",
    "stroke-dasharray": `0 ${PIE_CIRCUMFERENCE}`,
    transform: "rotate(-90 100 100)",
  });
  const ring = circle("pie-ring", { r: "94", fill: "none", stroke: "#111418", "stroke-width": "6" });
  const pupil = circle("pie-pupil", { r: "26", fill: "#111418" });
  pie.append(disc, wedge, ring, pupil);

  const time = el("div", "card-time");
  const timeValue = el("span", "card-time-value");
  const timeUnit = el("span", "card-unit");
  time.append(timeValue, timeUnit);

  const dots = el("div", "card-dots");
  dots.setAttribute("role", "img");
  const dotEls = [];

  const status = el("div", "card-status");
  const spacer = el("div", "card-spacer");
  const action = el("button", "card-action");
  action.type = "button";

  article.append(header, pie, time, dots, status, spacer, action);

  return { article, name, protocol, remove, pie, wedge, time, timeValue, timeUnit, dots, dotEls, status, action };
}

function setText(e, text) {
  if (e.textContent !== text) e.textContent = text;
}

function updateCard(refs, p, now) {
  const v = cardView(p, now);

  refs.article.classList.toggle("red", v.red);
  setText(refs.name, p.label);
  setText(refs.protocol, v.protocolLabel);

  refs.wedge.setAttribute("stroke-dasharray", `${(v.fraction * PIE_CIRCUMFERENCE).toFixed(1)} ${PIE_CIRCUMFERENCE}`);
  refs.pie.setAttribute("aria-label", v.red ? "Czas minął" : `Upłynęło ${Math.round(v.fraction * 100)}% czasu`);

  setText(refs.timeValue, v.time.value);
  setText(refs.timeUnit, v.time.unit);

  // The protocol never changes, so the dots are made once.
  while (refs.dotEls.length < v.dots.total) {
    const dot = el("span", "dot");
    refs.dotEls.push(dot);
    refs.dots.append(dot);
  }
  refs.dotEls.forEach((dot, i) => dot.classList.toggle("filled", i < v.dots.filled));
  refs.dots.setAttribute("aria-label", `Podano ${v.dots.filled} z ${v.dots.total} kropli`);

  setText(refs.status, v.status);
  setText(refs.action, v.action);
}

function render(state) {
  latest = state;
  const seen = new Set();
  for (const p of state.patients) {
    seen.add(p.id);
    let refs = cards.get(p.id);
    if (!refs) {
      // Patients come in arrival order and new ones are always last, so
      // appending keeps the cards in the same order as the payload.
      refs = createCard();
      cards.set(p.id, refs);
      grid.append(refs.article);
      updateCard(refs, p, state.now);
      onCardCreated(refs, p.id);
    } else {
      updateCard(refs, p, state.now);
    }
  }
  for (const [id, refs] of cards) {
    if (!seen.has(id)) {
      refs.article.remove();
      cards.delete(id);
    }
  }
  updateEmpty();
}

function updateEmpty() {
  empty.hidden = cards.size > 0 || !settingsView.hidden;
}

let gotEvent = false;
listen("state", (event) => {
  gotEvent = true;
  render(event.payload);
});
// Ask for the current state right away, so we don't miss the first event.
// If an event got here first, the reply is older than what is drawn.
invoke("get_state").then((state) => {
  if (!gotEvent) render(state);
});

// ---- Add row -------------------------------------------------------------

let invalidTimer;
function flashInvalid() {
  nameInput.classList.add("invalid");
  clearTimeout(invalidTimer);
  invalidTimer = setTimeout(() => nameInput.classList.remove("invalid"), 1000);
}

// True while an add_patient call is on its way, so a double-click on an
// add button cannot add the same patient twice.
let adding = false;

async function addPatient(protocol) {
  if (adding) return;
  const label = nameInput.value.trim();
  if (!label) {
    flashInvalid();
  } else {
    adding = true;
    try {
      await invoke("add_patient", { label, protocol });
      nameInput.value = "";
    } catch (err) {
      // Rust refused the label (e.g. too long); keep it so it can be fixed.
      flashInvalid();
      console.error(err);
    } finally {
      adding = false;
    }
  }
  nameInput.focus();
}

addOne.addEventListener("click", () => addPatient("one"));
addThree.addEventListener("click", () => addPatient("three"));

function setIntervalLabel(minutes) {
  addThree.textContent = `3 krople co ${minutes} min`;
}

// ---- Settings view -------------------------------------------------------
//
// Shown in place of the cards. Each field saves on "change". Rust applies
// new durations only to phases that start after the change.

const intervalInput = $("interval-minutes");
const waitInput = $("wait-minutes");
const autostartBox = $("autostart");
const settingsStatus = $("settings-status");

async function loadSettings() {
  try {
    const s = await invoke("get_settings");
    intervalInput.value = s.interval_minutes;
    waitInput.value = s.wait_minutes;
    autostartBox.checked = s.autostart;
    setIntervalLabel(s.interval_minutes);
    settingsStatus.textContent = "";
  } catch (err) {
    settingsStatus.textContent = "Nie udało się wczytać ustawień";
    console.error(err);
  }
}
// Once at startup too, for the "3 krople co {interval} min" label.
loadSettings();

// Saves a whole number of minutes in 1..max; returns true when saved.
async function saveMinutes(input, command, max) {
  const minutes = Number(input.value);
  if (input.value.trim() === "" || !Number.isInteger(minutes) || minutes < 1 || minutes > max) {
    settingsStatus.textContent = `Podaj liczbę od 1 do ${max}`;
    return false;
  }
  try {
    await invoke(command, { minutes });
    settingsStatus.textContent = "Zapisano";
    return true;
  } catch (err) {
    settingsStatus.textContent = "Nie udało się zapisać";
    console.error(err);
    return false;
  }
}

intervalInput.addEventListener("change", async () => {
  if (await saveMinutes(intervalInput, "set_interval_minutes", 60)) {
    setIntervalLabel(Number(intervalInput.value));
  }
});
waitInput.addEventListener("change", () => saveMinutes(waitInput, "set_wait_minutes", 120));

autostartBox.addEventListener("change", async () => {
  try {
    await invoke("set_autostart", { enabled: autostartBox.checked });
    settingsStatus.textContent = "Zapisano";
  } catch (err) {
    autostartBox.checked = !autostartBox.checked;
    settingsStatus.textContent = "Nie udało się zapisać";
    console.error(err);
  }
});

// "Zakończ" is a normal click: the only way to quit the app.
$("btn-quit").addEventListener("click", () => invoke("quit_app").catch(logError));

function showSettings(open) {
  settingsView.hidden = !open;
  grid.hidden = open;
  updateEmpty();
  // The add row stays visible but cannot be used while settings are open.
  for (const control of [nameInput, addOne, addThree]) control.disabled = open;
  if (open) {
    loadSettings();
    intervalInput.focus();
  } else {
    focusName();
  }
}

$("btn-settings").addEventListener("click", () => showSettings(settingsView.hidden));
$("btn-back").addEventListener("click", () => showSettings(false));

// ---- Top bar and focus ---------------------------------------------------

$("btn-minimize").addEventListener("click", () => getCurrentWindow().minimize());

// The name field is ready for typing whenever the window comes up, but not
// while the settings view is open.
function focusName() {
  if (settingsView.hidden) nameInput.focus();
}
focusName();
getCurrentWindow().onFocusChanged(({ payload: focused }) => {
  if (focused) focusName();
});
