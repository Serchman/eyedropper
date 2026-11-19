import { test } from "node:test";
import assert from "node:assert/strict";
import { formatTime, formatOverdue } from "./format.js";

test("formatTime shows running time as m:ss", () => {
  assert.equal(formatTime(61_000), "1:01");
});

test("formatOverdue shows whole minutes from +0", () => {
  assert.deepEqual(formatOverdue(0), { value: "+0", unit: "min" });
  assert.deepEqual(formatOverdue(59_999), { value: "+0", unit: "min" });
  assert.deepEqual(formatOverdue(60_000), { value: "+1", unit: "min" });
  assert.deepEqual(formatOverdue(125_000), { value: "+2", unit: "min" });
});

test("formatOverdue never goes negative", () => {
  assert.deepEqual(formatOverdue(-5_000), { value: "+0", unit: "min" });
});
