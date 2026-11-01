// Tracks the current contract (who holds it, its value, trump and coinche) from the calls of an auction
// or from a board, so the page can show a badge at the bidder's seat.
// A contract is `{ player, value, suit, coinched }`; `value` is null when trump was chosen without a bid.

// Applies one server call string to the contract and returns the new contract.
function contractAfterCall(contract, player, call) {
  if (!call || /^pass$/i.test(call)) return contract;
  if (/^coinche/i.test(call)) return contract ? { ...contract, coinched: true } : contract;
  const bid = /^(\d+)\s+(.+)$/.exec(call);
  if (bid) return { player, value: Number(bid[1]), suit: bid[2], coinched: false };
  return { player, value: null, suit: call, coinched: false };
}

// Folds the calls of an auction, in order, into the contract they leave.
function contractFromAuction(calls) {
  return calls.reduce((contract, { playerName, call }) => contractAfterCall(contract, playerName, call), null);
}

// Reads the contract of a board, or null when no one has declared.
function contractFromBoard(board) {
  const index = board.declaringPlayerIndex;
  if (index == null || index < 0) return null;
  return {
    player: board.seats[index].name,
    value: board.variant === "CONTREE" ? board.contractValue : null,
    suit: board.trump,
    coinched: Boolean(board.coinched),
  };
}

if (typeof module !== "undefined") module.exports = { contractAfterCall, contractFromAuction, contractFromBoard };
