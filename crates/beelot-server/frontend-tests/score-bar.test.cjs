const test = require("node:test");
const assert = require("node:assert/strict");
const { scoreBarModel, scoreBarAnnouncement, countValue } = require("../static/score-bar.js");

const MATCH = { northSouth: 420, eastWest: 310, target: 1000 };

test("no match gives no model", () => {
  assert.equal(scoreBarModel({ match: null, board: null, myTeam: 0 }), null);
  assert.equal(scoreBarModel({ match: undefined, board: undefined, myTeam: 0 }), null);
});

test("auction has no round points", () => {
  const model = scoreBarModel({ match: MATCH, board: null, myTeam: 0 });
  assert.deepEqual(
    model.teams.map((t) => [t.name, t.matchScore, t.roundPoints]),
    [
      ["North–South", 420, null],
      ["East–West", 310, null],
    ],
  );
  assert.equal(model.target, 1000);
  assert.equal(model.complete, false);
});

test("play shows round points", () => {
  const board = { northSouthScore: 62, eastWestScore: 35 };
  const model = scoreBarModel({ match: MATCH, board, myTeam: 0 });
  assert.equal(model.teams[0].roundPoints, 62);
  assert.equal(model.teams[1].roundPoints, 35);
});

test("my team", () => {
  const mine = (myTeam) => scoreBarModel({ match: MATCH, board: null, myTeam }).teams.map((t) => t.mine);
  assert.deepEqual(mine(0), [true, false]);
  assert.deepEqual(mine(1), [false, true]);
  assert.deepEqual(mine(null), [false, false]);
});

test("complete match", () => {
  const model = scoreBarModel({ match: { ...MATCH, complete: true }, board: null, myTeam: null });
  assert.equal(model.complete, true);
});

test("announcement", () => {
  const t = (key, ...a) => key.replace(/\{(\d)\}/g, (_, i) => a[i]);
  const format = (n) => n.toLocaleString("en");
  const board = { northSouthScore: 62, eastWestScore: 35 };
  const model = scoreBarModel({ match: { northSouth: 10420, eastWest: 310, target: 1000 }, board, myTeam: 0 });
  const text = scoreBarAnnouncement(model, t, format);
  assert.equal(text, "Match score: North–South 10,420, East–West 310, to 1,000.");
  assert.ok(!text.includes("62"));
});

test("progress is the share of the target", () => {
  const model = scoreBarModel({ match: { northSouth: 500, eastWest: 250, target: 1000 }, board: null, myTeam: 0 });
  assert.equal(model.teams[0].progress, 0.5);
  assert.equal(model.teams[1].progress, 0.25);
});

test("progress is clamped", () => {
  const model = scoreBarModel({ match: { northSouth: 1200, eastWest: -50, target: 1000 }, board: null, myTeam: 0 });
  assert.equal(model.teams[0].progress, 1);
  assert.equal(model.teams[1].progress, 0);
});

test("progress without a usable target is 0", () => {
  for (const target of [0, null, undefined]) {
    const model = scoreBarModel({ match: { northSouth: 500, eastWest: 250, target }, board: null, myTeam: 0 });
    assert.deepEqual(model.teams.map((t) => t.progress), [0, 0]);
  }
});

test("countValue endpoints", () => {
  assert.equal(countValue(100, 250, 0), 100);
  assert.equal(countValue(100, 250, 1), 250);
  assert.equal(countValue(100, 250, -1), 100);
  assert.equal(countValue(100, 250, 2), 250);
});

test("countValue never decreases", () => {
  let previous = 100;
  for (let i = 0; i <= 10; i++) {
    const value = countValue(100, 250, i / 10);
    assert.ok(value >= previous);
    assert.ok(value <= 250);
    previous = value;
  }
});

test("countValue with no rise returns to", () => {
  assert.equal(countValue(300, 300, 0.5), 300);
  assert.equal(countValue(300, 100, 0.5), 100);
});

test("countValue returns integers", () => {
  for (let i = 0; i <= 10; i++) assert.ok(Number.isInteger(countValue(0, 77, i / 10)));
});
