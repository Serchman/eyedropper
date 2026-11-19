// Press-and-hold buttons. A card action only happens after the button has
// been held down for HOLD_MS, so a stray click never marks a drop or
// removes a patient. A plain click does nothing.
//
// While held, the button has the class "holding"; the CSS draws a fill
// that grows from the left over var(--hold-ms) (keep it equal to HOLD_MS).

export const HOLD_MS = 1500;

const isHoldKey = (e) => e.key === " " || e.key === "Enter";

export function holdToConfirm(button, onConfirm) {
  let timer = null;

  function start() {
    if (timer !== null) return; // already holding (e.g. mouse and key)
    button.classList.add("holding");
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
