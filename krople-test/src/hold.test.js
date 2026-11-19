import { test } from "node:test";
import assert from "node:assert/strict";
import { HOLD_MS, holdToConfirm } from "./hold.js";

// A stand-in for a <button>: real events (Node's EventTarget), a classList
// and isConnected. Enough for holdToConfirm, no DOM library needed.
class FakeButton extends EventTarget {
  constructor() {
    super();
    const classes = new Set();
    this.classList = {
      add: (c) => classes.add(c),
      remove: (c) => classes.delete(c),
      contains: (c) => classes.has(c),
    };
    this.isConnected = true;
  }
  get holding() {
    return this.classList.contains("holding");
  }
}

function fire(target, type, props = {}) {
  const e = new Event(type, { cancelable: true });
  Object.assign(e, props);
  target.dispatchEvent(e);
  return e;
}
const press = (b) => fire(b, "pointerdown", { button: 0, pointerId: 1 });

function setup(t) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const button = new FakeButton();
  let confirmed = 0;
  holdToConfirm(button, () => confirmed++);
  return { button, count: () => confirmed };
}

test("hold time is 1500 ms", () => {
  assert.equal(HOLD_MS, 1500);
});

test("a full pointer hold confirms once and resets", (t) => {
  const { button, count } = setup(t);
  press(button);
  assert.ok(button.holding);
  t.mock.timers.tick(HOLD_MS - 1);
  assert.equal(count(), 0);
  t.mock.timers.tick(1);
  assert.equal(count(), 1);
  assert.ok(!button.holding);
  // Still pressed, more time passes: nothing more happens.
  t.mock.timers.tick(HOLD_MS * 3);
  fire(button, "pointerup");
  assert.equal(count(), 1);
});

test("a quick click does nothing", (t) => {
  const { button, count } = setup(t);
  press(button);
  fire(button, "pointerup");
  fire(button, "click");
  t.mock.timers.tick(HOLD_MS * 2);
  assert.equal(count(), 0);
  assert.ok(!button.holding);
});

for (const type of ["pointerup", "pointerleave", "pointercancel", "blur"]) {
  test(`${type} before the end cancels and resets`, (t) => {
    const { button, count } = setup(t);
    press(button);
    t.mock.timers.tick(HOLD_MS - 100);
    fire(button, type);
    assert.ok(!button.holding);
    t.mock.timers.tick(HOLD_MS);
    assert.equal(count(), 0);
  });
}

test("only the primary mouse button starts a hold", (t) => {
  const { button, count } = setup(t);
  fire(button, "pointerdown", { button: 2, pointerId: 1 });
  assert.ok(!button.holding);
  t.mock.timers.tick(HOLD_MS * 2);
  assert.equal(count(), 0);
});

for (const key of [" ", "Enter"]) {
  test(`holding ${key === " " ? "Space" : key} confirms once; early keyup cancels`, (t) => {
    const { button, count } = setup(t);
    const down = fire(button, "keydown", { key, repeat: false });
    assert.ok(down.defaultPrevented, "keydown default (the click) is prevented");
    assert.ok(button.holding);
    // Auto-repeat keydowns while held neither restart nor add a confirm.
    t.mock.timers.tick(1000);
    fire(button, "keydown", { key, repeat: true });
    t.mock.timers.tick(HOLD_MS - 1000);
    assert.equal(count(), 1);
    fire(button, "keyup", { key });
    assert.equal(count(), 1);

    fire(button, "keydown", { key, repeat: false });
    t.mock.timers.tick(500);
    fire(button, "keyup", { key });
    assert.ok(!button.holding);
    t.mock.timers.tick(HOLD_MS);
    assert.equal(count(), 1);
  });
}

test("other keys do not start a hold", (t) => {
  const { button, count } = setup(t);
  fire(button, "keydown", { key: "a", repeat: false });
  assert.ok(!button.holding);
  t.mock.timers.tick(HOLD_MS * 2);
  assert.equal(count(), 0);
});

test("a second press during a hold does not restart the timer", (t) => {
  const { button, count } = setup(t);
  press(button);
  t.mock.timers.tick(1000);
  fire(button, "keydown", { key: " ", repeat: false });
  t.mock.timers.tick(HOLD_MS - 1000);
  assert.equal(count(), 1);
});

test("a button removed from the page mid-hold does not confirm", (t) => {
  const { button, count } = setup(t);
  press(button);
  button.isConnected = false;
  t.mock.timers.tick(HOLD_MS);
  assert.equal(count(), 0);
  assert.ok(!button.holding);
});
