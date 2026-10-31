const test = require("node:test");
const assert = require("node:assert/strict");
const { resolveTheme, THEMES } = require("../static/theme.js");

test("lists the four themes with comic first", () => {
  assert.deepEqual(THEMES, ["comic", "classic-green", "midnight", "sunset"]);
});

test("a stored known theme is kept", () => {
  for (const theme of THEMES) assert.equal(resolveTheme(theme), theme);
});

test("nothing stored means comic", () => {
  assert.equal(resolveTheme(null), "comic");
  assert.equal(resolveTheme(undefined), "comic");
});

test("an unknown stored value means comic", () => {
  assert.equal(resolveTheme("banana"), "comic");
  assert.equal(resolveTheme(""), "comic");
  assert.equal(resolveTheme("MIDNIGHT"), "comic");
});
