// Builds the score bar shown above the table: the match score of each team, the points of the round in
// progress and the score to reach, so the page can draw the bar and announce it to screen readers.
// A model is `{ teams: [north-south, east-west], target, complete }`; a team is
// `{ name, matchScore, roundPoints, mine }`, with `roundPoints` null while no board is being played.

// Reads the match and board into the model, or null when there is no match.
// `myTeam` is 0 for North–South, 1 for East–West and null when the viewer sits in neither.
function scoreBarModel({ match, board, myTeam }) {
  if (match == null) return null;
  const round = (points) => (board == null ? null : points);
  return {
    teams: [
      { name: "North–South", matchScore: match.northSouth, roundPoints: round(board?.northSouthScore), mine: myTeam === 0 },
      { name: "East–West", matchScore: match.eastWest, roundPoints: round(board?.eastWestScore), mine: myTeam === 1 },
    ],
    target: match.target,
    complete: Boolean(match.complete),
  };
}

// Words the match score for a live region; `t` translates and `format` formats numbers.
// Round points change too often, so they are left out.
function scoreBarAnnouncement(model, t, format) {
  const [northSouth, eastWest] = model.teams;
  return t(
    "Match score: North–South {0}, East–West {1}, to {2}.",
    format(northSouth.matchScore),
    format(eastWest.matchScore),
    format(model.target),
  );
}

if (typeof module !== "undefined") module.exports = { scoreBarModel, scoreBarAnnouncement };
