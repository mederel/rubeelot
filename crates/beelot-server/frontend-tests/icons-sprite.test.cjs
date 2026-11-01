const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const svg = fs.readFileSync(path.join(__dirname, "../static/icons.svg"), "utf8");
const symbols = [...svg.matchAll(/<symbol\b([^>]*)>([\s\S]*?)<\/symbol>/g)].map((m) => ({
  attrs: m[1],
  body: m[2],
  id: /\bid="([^"]+)"/.exec(m[1])?.[1],
}));

test("has the dealer symbol with a 24 by 24 viewBox", () => {
  const dealer = symbols.find((s) => s.id === "icon-dealer");
  assert.ok(dealer, "icon-dealer is missing");
  assert.match(dealer.attrs, /viewBox="0 0 24 24"/);
});

test("every symbol paints with currentColor and no fixed colour", () => {
  for (const s of symbols) {
    assert.match(s.body, /currentColor/, s.id);
    assert.doesNotMatch(s.body, /#[0-9a-fA-F]{3,8}\b|rgb\(/, s.id);
  }
});

test("has no text, images or external references", () => {
  assert.doesNotMatch(svg, /<text|<image|<script|href="http|url\(http/);
});
