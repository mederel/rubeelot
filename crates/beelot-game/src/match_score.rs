//! Team scores added up over the rounds of a match. The first team to reach the match target
//! (1,000 points unless the players chose another of [`TARGETS`]) wins; when both teams reach it
//! in the same round, the higher score wins, and an exact tie is settled by playing another
//! round. The rounds are kept with their contracts.

use serde::{Deserialize, Serialize};

use crate::card::Team;
use crate::scoring::{RoundResult, RoundSummary};

/// The points a match is played to when the players choose nothing else.
pub const DEFAULT_TARGET: i32 = 1_000;
/// The match targets players may choose (US-062).
pub const TARGETS: [i32; 4] = [500, 1_000, 1_500, 2_000];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchScore {
    rounds: Vec<RoundSummary>,
    north_south: i32,
    east_west: i32,
    complete: bool,
    target: i32,
}

impl Default for MatchScore {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchScore {
    /// A match to 1,000 points.
    pub fn new() -> Self {
        Self::to(DEFAULT_TARGET).expect("1,000 is a match target")
    }

    /// A match to `target`, which must be one of [`TARGETS`].
    pub fn to(target: i32) -> Option<Self> {
        TARGETS.contains(&target).then(|| Self {
            rounds: Vec::new(),
            north_south: 0,
            east_west: 0,
            complete: false,
            target,
        })
    }

    /// Adds a finished round; returns true when this round ends the match, which happens once.
    pub fn record_summary(&mut self, round: RoundSummary) -> bool {
        if self.complete {
            return false;
        }
        let result = round.result.clone();
        self.rounds.push(round);
        self.record(&result);
        self.complete
    }

    pub fn record(&mut self, result: &RoundResult) {
        if self.complete {
            return;
        }
        self.north_south = self.north_south.wrapping_add(result.north_south_awarded);
        self.east_west = self.east_west.wrapping_add(result.east_west_awarded);
        self.complete = self.north_south.max(self.east_west) >= self.target
            && self.north_south != self.east_west;
    }

    pub fn rounds(&self) -> &[RoundSummary] {
        &self.rounds
    }

    pub fn north_south(&self) -> i32 {
        self.north_south
    }

    pub fn east_west(&self) -> i32 {
        self.east_west
    }

    pub fn target(&self) -> i32 {
        self.target
    }

    pub fn complete(&self) -> bool {
        self.complete
    }

    pub fn winner(&self) -> Option<Team> {
        if !self.complete {
            return None;
        }
        Some(if self.north_south > self.east_west {
            Team::NorthSouth
        } else {
            Team::EastWest
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Suit, Team};

    fn round(north_south_awarded: i32, east_west_awarded: i32) -> RoundResult {
        RoundResult {
            north_south_card_points: 0,
            east_west_card_points: 0,
            north_south_dix_de_der: 0,
            east_west_dix_de_der: 0,
            north_south_belote_bonus: 0,
            east_west_belote_bonus: 0,
            contract_made: true,
            north_south_awarded,
            east_west_awarded,
            capot_team: None,
            north_south_announcements: 0,
            east_west_announcements: 0,
            announcements: vec![],
        }
    }

    fn summary(result: RoundResult) -> RoundSummary {
        RoundSummary {
            declaring_team: Team::NorthSouth,
            trump: Suit::Hearts,
            contract_value: Some(80),
            coinched: false,
            result,
        }
    }

    #[test]
    fn serializes_its_fields_in_camel_case() {
        let json = serde_json::to_value(MatchScore::new()).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["complete", "eastWest", "northSouth", "rounds", "target"]
        );
    }

    #[test]
    fn plays_to_one_thousand_points_by_default() {
        assert_eq!(MatchScore::new().target(), 1_000);
        assert_eq!(MatchScore::default().target(), 1_000);
    }

    #[test]
    fn ends_the_match_when_a_team_reaches_each_target() {
        for target in TARGETS {
            let mut score = MatchScore::to(target).unwrap();
            score.record(&round(target - 1, 0));
            assert!(!score.complete(), "{} of {target}", target - 1);
            score.record(&round(1, 0));
            assert!(score.complete(), "{target} of {target}");
            assert_eq!(score.target(), target);
        }
    }

    #[test]
    fn a_tie_at_five_hundred_plays_on() {
        let mut score = MatchScore::to(500).unwrap();
        score.record(&round(520, 520));
        assert!(!score.complete());
        score.record(&round(0, 162));
        assert_eq!(score.winner(), Some(Team::EastWest));
    }

    #[test]
    fn refuses_a_target_that_is_not_offered() {
        for target in [0, 750, 1_001, 3_000, -500] {
            assert_eq!(MatchScore::to(target), None, "{target}");
        }
    }

    #[test]
    fn ends_the_match_when_a_team_reaches_one_thousand_points() {
        let mut score = MatchScore::new();
        score.record(&round(1_000, 0));
        assert_eq!(score.north_south(), 1_000);
        assert!(score.complete());
        assert_eq!(score.winner(), Some(Team::NorthSouth));
    }

    #[test]
    fn the_match_goes_on_below_one_thousand_points() {
        let mut score = MatchScore::new();
        score.record(&round(500, 162));
        score.record(&round(490, 0));
        assert_eq!(score.north_south(), 990);
        assert!(!score.complete());
        assert_eq!(score.winner(), None);
    }

    #[test]
    fn when_both_teams_pass_one_thousand_the_higher_score_wins() {
        let mut score = MatchScore::new();
        score.record(&round(950, 900));
        score.record(&round(60, 162));
        assert!(score.complete());
        assert_eq!(score.winner(), Some(Team::EastWest));
    }

    #[test]
    fn an_exact_tie_above_one_thousand_is_settled_by_another_round() {
        let mut score = MatchScore::new();
        score.record(&round(950, 950));
        score.record(&round(81, 81));
        assert!(!score.complete(), "a tie at 1,031 has no winner yet");
        score.record(&round(162, 0));
        assert!(score.complete());
        assert_eq!(score.winner(), Some(Team::NorthSouth));
    }

    #[test]
    fn rounds_after_the_end_of_the_match_are_ignored() {
        let mut score = MatchScore::new();
        score.record(&round(0, 1_000));
        score.record(&round(500, 0));
        assert_eq!(score.north_south(), 0);
        assert_eq!(score.winner(), Some(Team::EastWest));
    }

    #[test]
    fn keeps_the_rounds_and_says_once_that_the_match_has_ended() {
        let mut score = MatchScore::new();
        let first = summary(round(600, 0));
        let second = summary(round(500, 100));

        assert!(!score.record_summary(first.clone()));
        assert!(
            score.record_summary(second.clone()),
            "this round ends the match"
        );
        assert!(
            !score.record_summary(summary(round(0, 162))),
            "a won match takes no more rounds"
        );

        assert_eq!(score.rounds(), [first, second]);
        assert_eq!(score.north_south(), 1_100);
    }
}
