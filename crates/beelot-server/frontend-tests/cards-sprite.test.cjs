const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const svg = fs.readFileSync(path.join(__dirname, "../static/cards.svg"), "utf8");
const symbols = [...svg.matchAll(/<symbol\b([^>]*)>([\s\S]*?)<\/symbol>/g)].map((m) => ({
  attrs: m[1],
  body: m[2],
  id: /\bid="([^"]+)"/.exec(m[1])?.[1],
}));

// Fixed colours the portraits may use besides the card tokens (skin, gold, and per-suit accents).
const ACCENTS = [
  "#f6c9a0", "#e8a87c", "#ffd23f", "#f4a300", "#ffffff", "#8a4f20", "#5b2b82",
  "#ff5d8f", "#c9184a", "#2ec4b6", "#7a8794", "#c0c8d0", "#3a7d44", "#ff9f1c",
  "#4cc9f0", "#a05a2c", "#2b2d42",
];

test("has exactly twelve court symbols with the expected ids", () => {
  const expected = [];
  for (const rank of ["J", "Q", "K"]) {
    for (const suit of ["clubs", "diamonds", "hearts", "spades"]) expected.push(`court-${rank}-${suit}`);
  }
  assert.deepEqual(symbols.map((s) => s.id).sort(), expected.sort());
});

test("every symbol has viewBox 0 0 100 130", () => {
  for (const s of symbols) assert.match(s.attrs, /viewBox="0 0 100 130"/, s.id);
});

test("no symbol contains text, images or external references", () => {
  for (const s of symbols) {
    assert.doesNotMatch(s.body, /<text|<image|<script|href="https?:|href="\/|url\(["']?https?:/i, s.id);
  }
});

test("every colour is a token or an allowed accent", () => {
  for (const s of symbols) {
    for (const m of s.body.matchAll(/\b(?:fill|stroke)="([^"]+)"/g)) {
      const value = m[1].toLowerCase();
      const ok = value === "none" || value === "currentcolor" || value.startsWith("var(--") || ACCENTS.includes(value);
      assert.ok(ok, `${s.id} uses ${value}`);
    }
  }
});

test("the twelve portraits are all different", () => {
  const bodies = symbols.map((s) => s.body.replace(/\s+/g, " ").trim());
  assert.equal(new Set(bodies).size, bodies.length);
});
