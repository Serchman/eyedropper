import { test } from "node:test";
import assert from "node:assert/strict";
import { isOutdated } from "./state.js";

const s = (rev, now) => ({ rev, now, patients: [], ready_count: 0 });

test("anything is newer than nothing", () => {
  assert.equal(isOutdated(null, s(0, 1000)), false);
});

test("a lower rev is outdated, even with a newer now", () => {
  // A tick snapshot (rev 4) arriving after a command's broadcast (rev 5).
  assert.equal(isOutdated(s(5, 2000), s(4, 2100)), true);
});

test("a higher rev is drawn", () => {
  assert.equal(isOutdated(s(4, 2000), s(5, 1900)), false);
});

test("the same rev with a newer or equal now is drawn (a plain tick)", () => {
  assert.equal(isOutdated(s(5, 2000), s(5, 3000)), false);
  assert.equal(isOutdated(s(5, 2000), s(5, 2000)), false);
});

test("the same rev with an older now is outdated (a late get_state reply)", () => {
  assert.equal(isOutdated(s(5, 3000), s(5, 2000)), true);
});
