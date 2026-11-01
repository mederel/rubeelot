const test = require("node:test");
const assert = require("node:assert/strict");
const { splitSuitText } = require("../static/suit-text.js");

const FR = { "trèfle": "clubs", "carreau": "diamonds", "cœur": "hearts", "pique": "spades" };
const EN = { clubs: "clubs", diamonds: "diamonds", hearts: "hearts", spades: "spades" };
const NL = { klaveren: "clubs", ruiten: "diamonds", harten: "hearts", schoppen: "spades" };

test("splits an English sentence", () => {
  assert.deepEqual(splitSuitText("Hearts are trump", EN), [{ suit: "hearts", word: "Hearts" }, " are trump"]);
});

test("splits French and Dutch sentences", () => {
  assert.deepEqual(splitSuitText("Cœur est atout", FR), [{ suit: "hearts", word: "Cœur" }, " est atout"]);
  assert.deepEqual(splitSuitText("Harten is troef", NL), [{ suit: "hearts", word: "Harten" }, " is troef"]);
});

test("matches whole words only", () => {
  assert.deepEqual(splitSuitText("piquant", FR), ["piquant"]);
  assert.deepEqual(splitSuitText("clubhouse", EN), ["clubhouse"]);
});

test("is not fooled by accented neighbours", () => {
  assert.deepEqual(splitSuitText("un trèfle", FR), ["un ", { suit: "clubs", word: "trèfle" }]);
  assert.deepEqual(splitSuitText("trèflerie", FR), ["trèflerie"]);
});

test("keeps the original case", () => {
  assert.deepEqual(splitSuitText("CLUBS", EN), [{ suit: "clubs", word: "CLUBS" }]);
});

test("handles two suits and edges", () => {
  const parts = splitSuitText("Spades are trump and clubs were led.", EN);
  assert.equal(parts.length, 4);
  assert.deepEqual(parts[0], { suit: "spades", word: "Spades" });
  assert.deepEqual(parts[2], { suit: "clubs", word: "clubs" });
  assert.equal(parts[3], " were led.");
});

test("returns plain text unchanged", () => {
  assert.deepEqual(splitSuitText("Your turn", EN), ["Your turn"]);
  assert.deepEqual(splitSuitText("", EN), []);
});
