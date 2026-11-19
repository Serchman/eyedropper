import { test } from "node:test";
import assert from "node:assert/strict";
import { cardView, PIE_CIRCUMFERENCE } from "./card.js";

const now = 1_000_000;
const MIN = 60_000;

// A patient whose current phase started `startedAgo` ms before `now` and
// ends `endsIn` ms after it (negative = already over).
function patient(protocol, drops_given, startedAgo, endsIn) {
  return {
    id: 1,
    label: "A.K.",
    protocol,
    drops_given,
    phase_started_at: now - startedAgo,
    phase_ends_at: now + endsIn,
    ready: endsIn <= 0,
  };
}

test("pie circumference matches the mockup", () => {
  assert.equal(PIE_CIRCUMFERENCE, 295.3);
});

test("1 kropla, waiting", () => {
  const v = cardView(patient("one", 1, 5 * MIN, 10 * MIN), now);
  assert.equal(v.red, false);
  assert.equal(v.isWait, true);
  assert.equal(v.protocolLabel, "1 kropla");
  assert.equal(v.status, "Czeka");
  assert.equal(v.action, "Zbadany");
  assert.deepEqual(v.dots, { total: 1, filled: 1 });
  assert.deepEqual(v.time, { value: "10:00", unit: "" });
  assert.ok(Math.abs(v.fraction - 1 / 3) < 1e-9);
});

test("1 kropla, done", () => {
  const v = cardView(patient("one", 1, 17 * MIN, -2 * MIN), now);
  assert.equal(v.red, true);
  assert.equal(v.status, "Gotowy do badania");
  assert.equal(v.action, "Zbadany");
  assert.equal(v.fraction, 1);
  assert.deepEqual(v.time, { value: "+2", unit: "min" });
  assert.deepEqual(v.dots, { total: 1, filled: 1 });
});

test("3 krople, drops 1 and 2, waiting", () => {
  for (const drops of [1, 2]) {
    const v = cardView(patient("three", drops, 3 * MIN, 7 * MIN), now);
    assert.equal(v.red, false);
    assert.equal(v.isWait, false);
    assert.equal(v.protocolLabel, "3 krople");
    assert.equal(v.status, "Czeka");
    assert.equal(v.action, "Zakropione");
    assert.deepEqual(v.dots, { total: 3, filled: drops });
    assert.deepEqual(v.time, { value: "7:00", unit: "" });
  }
});

test("3 krople, drop due: red, dots given so far still shown", () => {
  const v = cardView(patient("three", 2, 10 * MIN, 0), now);
  assert.equal(v.red, true);
  assert.equal(v.isWait, false);
  assert.equal(v.status, "Kropić teraz");
  assert.equal(v.action, "Zakropione");
  assert.equal(v.fraction, 1);
  assert.deepEqual(v.time, { value: "+0", unit: "min" });
  assert.deepEqual(v.dots, { total: 3, filled: 2 });
});

test("3 krople, 3 drops, waiting", () => {
  const v = cardView(patient("three", 3, 1 * MIN, 14 * MIN), now);
  assert.equal(v.red, false);
  assert.equal(v.isWait, true);
  assert.equal(v.status, "Czeka");
  assert.equal(v.action, "Zbadany");
  assert.deepEqual(v.dots, { total: 3, filled: 3 });
});

test("3 krople, 3 drops, done", () => {
  const v = cardView(patient("three", 3, 16 * MIN, -1 * MIN), now);
  assert.equal(v.red, true);
  assert.equal(v.status, "Gotowy do badania");
  assert.equal(v.action, "Zbadany");
  assert.deepEqual(v.dots, { total: 3, filled: 3 });
  assert.deepEqual(v.time, { value: "+1", unit: "min" });
});

test("fraction clamps and never becomes NaN", () => {
  // now before the phase started
  assert.equal(cardView(patient("one", 1, -1 * MIN, 10 * MIN), now).fraction, 0);
  // halfway
  assert.equal(cardView(patient("one", 1, 5 * MIN, 5 * MIN), now).fraction, 0.5);
  // zero-length phase, not yet marked ready
  const p = { ...patient("one", 1, 0, 0), ready: false };
  assert.equal(cardView(p, now).fraction, 0);
});
