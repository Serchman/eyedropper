// Press-and-hold buttons. A card action (or "Zakończ") only happens after the
// button has been held down for HOLD_MS, so a stray click never marks a drop,
// removes a patient or quits the app. A plain click does nothing.
//
// For touch screens the CSS gives every hold button `touch-action: none`, so
// a held finger is not taken over as a scroll gesture (which would cancel
// the pointer and with it the hold).
//
// While held, the button has the class "holding"; the CSS draws a fill
// that grows from the left over var(--hold-ms) (keep it equal to HOLD_MS).

export const HOLD_MS = 1500;

const isHoldKey = (e) => e.key === " " || e.key === "Enter";

// Buttons being held right now, across all holdToConfirm buttons.
const held = new Set();

// True while any button is being held. Code that moves focus checks this
// first: taking focus away (blur) would cancel the hold.
export function holdInProgress() {
  return held.size > 0;
}

export function holdToConfirm(button, onConfirm) {
  let timer = null;

  function start() {
    if (timer !== null) return; // already holding (e.g. mouse and key)
    button.classList.add("holding");
    held.add(button);
    timer = setTimeout(() => {
      timer = null; // already fired, nothing to clear
      reset();
      // A card can disappear mid-hold (another window, or Rust removed it).
      if (button.isConnected) onConfirm();
    }, HOLD_MS);
  }

  function reset() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    held.delete(button);
    button.classList.remove("holding");
  }

  button.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return; // primary button only
    // Touch and pen capture the pointer by default, which would stop
    // "pointerleave" from cancelling the hold. Release it.
    if (button.hasPointerCapture?.(e.pointerId)) button.releasePointerCapture(e.pointerId);
    start();
  });
  for (const type of ["pointerup", "pointerleave", "pointercancel", "blur"]) {
    button.addEventListener(type, reset);
  }

  // A long touch press would open the context menu (and cancel the
  // pointer). Hold buttons have no menu.
  button.addEventListener("contextmenu", (e) => e.preventDefault());

  button.addEventListener("keydown", (e) => {
    if (!isHoldKey(e)) return;
    // Enter would otherwise "click" the button (and repeat while held).
    e.preventDefault();
    if (!e.repeat) start();
  });
  button.addEventListener("keyup", (e) => {
    if (isHoldKey(e)) reset();
  });
}
