const test = require("node:test");
const assert = require("node:assert/strict");
const { resolveEffectsLevel } = require("../static/effects.js");

test("a stored choice beats the system setting", () => {
  assert.equal(resolveEffectsLevel("off", null, false), "off");
  assert.equal(resolveEffectsLevel("reduced", null, false), "reduced");
  assert.equal(resolveEffectsLevel("full", null, true), "full");
});

test("without a stored choice the level follows prefers-reduced-motion", () => {
  assert.equal(resolveEffectsLevel(null, null, false), "full");
  assert.equal(resolveEffectsLevel(null, null, true), "reduced");
});

test("an invalid stored value is treated as unset", () => {
  assert.equal(resolveEffectsLevel("banana", null, true), "reduced");
  assert.equal(resolveEffectsLevel("banana", null, false), "full");
});

test("the legacy reduce-motion key migrates to reduced", () => {
  assert.equal(resolveEffectsLevel(null, "true", false), "reduced");
  assert.equal(resolveEffectsLevel(null, "false", false), "full");
});

test("the new key wins over the legacy key", () => {
  assert.equal(resolveEffectsLevel("full", "true", false), "full");
});
