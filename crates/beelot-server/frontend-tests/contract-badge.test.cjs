const test = require("node:test");
const assert = require("node:assert/strict");
const { contractAfterCall, contractFromAuction, contractFromBoard } = require("../static/contract-badge.js");

const AUCTION = [
  { playerName: "You", call: "90 Hearts" },
  { playerName: "Camille", call: "Pass" },
  { playerName: "Luc", call: "110 Hearts" },
  { playerName: "Manon", call: "Pass" },
];

test("a first bid makes the contract", () => {
  assert.deepEqual(contractAfterCall(null, "You", "90 Hearts"), {
    player: "You",
    value: 90,
    suit: "Hearts",
    coinched: false,
  });
});

test("a raise by another player moves the holder", () => {
  const c = contractAfterCall(null, "You", "90 Hearts");
  const raised = contractAfterCall(c, "Luc", "110 Hearts");
  assert.equal(raised.player, "Luc");
  assert.equal(raised.value, 110);
});

test("passes keep the contract and a first pass gives none", () => {
  const c = contractAfterCall(null, "You", "90 Hearts");
  assert.equal(contractAfterCall(c, "Camille", "Pass"), c);
  assert.equal(contractAfterCall(null, "Camille", "Pass"), null);
  assert.equal(contractAfterCall(c, "Camille", ""), c);
});

test("coinche marks the contract and keeps its holder", () => {
  const c = contractAfterCall(null, "You", "90 Hearts");
  assert.deepEqual(contractAfterCall(c, "Camille", "Coinche!"), { ...c, coinched: true });
  assert.equal(contractAfterCall(null, "Camille", "Coinche!"), null);
});

test("a chosen trump has no value", () => {
  assert.deepEqual(contractAfterCall(null, "Ana", "Spades"), {
    player: "Ana",
    value: null,
    suit: "Spades",
    coinched: false,
  });
});

test("tout atout and sans atout bids keep their name", () => {
  const all = contractAfterCall(null, "Ana", "100 All trumps");
  assert.equal(all.value, 100);
  assert.equal(all.suit, "All trumps");
  const none = contractAfterCall(null, "Ana", "80 No trumps");
  assert.equal(none.value, 80);
  assert.equal(none.suit, "No trumps");
});

test("an auction folds to its last bid", () => {
  assert.deepEqual(contractFromAuction(AUCTION), { player: "Luc", value: 110, suit: "Hearts", coinched: false });
  assert.equal(contractFromAuction(AUCTION.slice(0, 2)).player, "You");
  assert.equal(contractFromAuction([]), null);
});

test("a board gives its contract", () => {
  const board = {
    variant: "CONTREE",
    seats: [{ name: "Ana" }, { name: "Camille" }, { name: "Luc" }, { name: "Manon" }],
    declaringPlayerIndex: 2,
    contractValue: 100,
    trump: "Spades",
    coinched: true,
  };
  assert.deepEqual(contractFromBoard(board), { player: "Luc", value: 100, suit: "Spades", coinched: true });
  assert.equal(contractFromBoard({ ...board, variant: "CLASSIC" }).value, null);
  const { declaringPlayerIndex, ...without } = board;
  assert.equal(contractFromBoard(without), null);
});
