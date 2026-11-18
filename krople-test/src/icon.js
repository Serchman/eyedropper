// The floating icon window.
//
// It only draws what Rust sends in the "state" event. Turning red ONLY
// changes a CSS class: it never shows, focuses, moves or activates any
// window, so keystrokes typed into DrEryk keep going to DrEryk.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const badge = document.getElementById("badge");
const count = document.getElementById("count");

function render(state) {
  count.textContent = String(state.ready_count);
  badge.classList.toggle("ready", state.ready_count > 0);
  // Tell Rust what we now show, for the consistency log in the terminal.
  const readyIds = state.timers.filter((t) => t.ready).map((t) => t.id);
  invoke("report_shown", { readyIds }).catch(() => {});
}

listen("state", (event) => render(event.payload));
// Ask for the current state right away, so we don't miss the first event.
invoke("get_state").then(render);

// --- Click vs. drag -------------------------------------------------------
// We don't use the `data-tauri-drag-region` attribute here: it starts a
// window drag on every mouse-down, and then Windows takes over the mouse, so
// the "click" never arrives. Instead we start the drag ourselves, but only
// once the mouse has moved a few pixels with the button held down. If the
// button is released before that, it counts as a click.
const DRAG_THRESHOLD_PX = 4;
let pressAt = null;

badge.addEventListener("mousedown", (e) => {
  if (e.button === 0) pressAt = { x: e.screenX, y: e.screenY };
});

window.addEventListener("mousemove", (e) => {
  if (!pressAt) return;
  const moved = Math.hypot(e.screenX - pressAt.x, e.screenY - pressAt.y);
  if (moved >= DRAG_THRESHOLD_PX) {
    pressAt = null;
    getCurrentWindow().startDragging();
  }
});

window.addEventListener("mouseup", (e) => {
  if (e.button === 0 && pressAt) {
    pressAt = null;
    invoke("show_main");
  }
});

// No right-click menu on the icon.
window.addEventListener("contextmenu", (e) => e.preventDefault());
