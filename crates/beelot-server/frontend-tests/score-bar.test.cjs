const test = require("node:test");
const assert = require("node:assert/strict");
const { scoreBarModel, scoreBarAnnouncement } = require("../static/score-bar.js");

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
