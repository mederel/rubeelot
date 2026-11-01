// Builds the score bar shown above the table: the match score of each team, the points of the round in
// progress and the score to reach, so the page can draw the bar and announce it to screen readers.
// A model is `{ teams: [north-south, east-west], target, complete }`; a team is
// `{ name, matchScore, roundPoints, progress, mine }`, with `roundPoints` null while no board is being played
// and `progress` the match score as a share of the target, from 0 to 1.

// Reads the match and board into the model, or null when there is no match.
// `myTeam` is 0 for North–South, 1 for East–West and null when the viewer sits in neither.
function scoreBarModel({ match, board, myTeam }) {
  if (match == null) return null;
  const usableTarget = Number.isFinite(match.target) && match.target > 0;
  const progress = (score) =>
    usableTarget && Number.isFinite(score) ? Math.min(1, Math.max(0, score / match.target)) : 0;
  const round = (points) => (board == null ? null : points);
  return {
    teams: [
      { name: "North–South", matchScore: match.northSouth, roundPoints: round(board?.northSouthScore), progress: progress(match.northSouth), mine: myTeam === 0 },
      { name: "East–West", matchScore: match.eastWest, roundPoints: round(board?.eastWestScore), progress: progress(match.eastWest), mine: myTeam === 1 },
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

// The integer shown at progress `t` (0 to 1) of a count from `from` up to `to`, easing out.
// It never exceeds `to`, and shows `to` at once when there is nothing to count up.
function countValue(from, to, t) {
  if (from >= to || t >= 1) return to;
  if (!(t > 0)) return from;
  return from + Math.round((to - from) * (1 - (1 - t) ** 3));
}

if (typeof module !== "undefined") module.exports = { scoreBarModel, scoreBarAnnouncement, countValue };
