const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const svg = fs.readFileSync(path.join(__dirname, "../static/suits.svg"), "utf8");
const symbols = [...svg.matchAll(/<symbol\b([^>]*)>([\s\S]*?)<\/symbol>/g)].map((m) => ({
  attrs: m[1],
  body: m[2],
  id: /\bid="([^"]+)"/.exec(m[1])?.[1],
}));

test("has exactly the four suit symbols with the expected ids", () => {
  assert.deepEqual(
    symbols.map((s) => s.id).sort(),
    ["suit-clubs", "suit-diamonds", "suit-hearts", "suit-spades"],
  );
});

test("every symbol uses a 24 by 24 viewBox", () => {
  for (const s of symbols) assert.match(s.attrs, /viewBox="0 0 24 24"/, s.id);
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

test("the four shapes differ", () => {
  const bodies = new Set(symbols.map((s) => s.body.trim()));
  assert.equal(bodies.size, 4);
});
