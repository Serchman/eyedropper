// What one patient card shows, worked out from the Rust state payload.
// Pure (no DOM), so it can be tested with `npm test`. The table it follows
// is "Card behaviour" in docs/superpowers/specs/2026-11-19-krople-real-ui-design.md.

import { formatTime, formatOverdue } from "./format.js";

// Circumference of the r=47 circle that draws the pie wedge (2 * pi * 47).
export const PIE_CIRCUMFERENCE = 295.3;

export function cardView(p, now) {
  // "Wait" = the last phase before the exam: 1 kropla always, 3 krople
  // once all three drops are in. Its button removes the card ("Zbadany").
  const isWait = p.protocol === "one" || p.drops_given >= 3;
  // Rust decides red, so every window agrees on it.
  const red = p.ready;

  let fraction = 1;
  if (!red) {
    const span = p.phase_ends_at - p.phase_started_at;
    fraction = span > 0 ? Math.min(Math.max((now - p.phase_started_at) / span, 0), 1) : 0;
  }

  let status = "Czeka";
  if (red) status = isWait ? "Gotowy do badania" : "Kropić teraz";

  const total = p.protocol === "one" ? 1 : 3;

  return {
    red,
    isWait,
    protocolLabel: p.protocol === "one" ? "1 kropla" : "3 krople",
    status,
    action: isWait ? "Zbadany" : "Zakropione",
    fraction,
    time: red
      ? formatOverdue(now - p.phase_ends_at)
      : { value: formatTime(p.phase_ends_at - now), unit: "" },
    // Dots given so far stay visible in every state, also "Kropić teraz".
    dots: { total, filled: Math.min(p.drops_given, total) },
  };
}
