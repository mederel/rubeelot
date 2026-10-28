//! A player's results, from the stored matches in which their account held a seat. Contracts, coinches, capots,
//! belotes and points count for the player's team. Ports Java's `PlayerStatistics`, field names included.

use beelot_game::Team;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::matches::{StoredMatch, difficulty_code, mode_code, variant_code};
use crate::tables::finished::Mode;

const RECENT_MATCHES: usize = 20;

/// Matches played, won and lost, and the win rate in percent (null before the first match).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub key: &'static str,
    pub played: u32,
    pub won: u32,
    pub lost: u32,
    pub win_rate: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentMatch {
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    pub mode: &'static str,
    pub variant: &'static str,
    pub difficulty: Option<&'static str>,
    pub partner: String,
    pub opponents: Vec<String>,
    pub team_score: i32,
    pub opponent_score: i32,
    pub won: bool,
    /// The points the match was played to.
    pub target: i32,
}

/// Contracts the team took and made; coinches the team made against the opponents' contracts and how many defeated
/// them; coinches the opponents made against the team's contracts and how many defeated them; capots and belotes the
/// team scored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contracts {
    pub taken: u32,
    pub made: u32,
    pub coinches_by_team: u32,
    pub coinches_by_team_won: u32,
    pub coinches_against_team: u32,
    pub coinches_against_team_won: u32,
    pub capots: u32,
    pub belotes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatistics {
    pub overall: Record,
    pub by_variant: Vec<Record>,
    pub by_difficulty: Vec<Record>,
    pub recent_matches: Vec<RecentMatch>,
    pub contracts: Contracts,
    pub rounds: u32,
    pub average_points_per_round: Option<f64>,
}

struct Tally {
    key: &'static str,
    played: u32,
    won: u32,
}

impl Tally {
    fn new(key: &'static str) -> Self {
        Self {
            key,
            played: 0,
            won: 0,
        }
    }

    fn add(&mut self, won: bool) {
        self.played += 1;
        self.won += u32::from(won);
    }

    fn record(&self) -> Record {
        Record {
            key: self.key,
            played: self.played,
            won: self.won,
            lost: self.played - self.won,
            win_rate: (self.played > 0)
                .then(|| (100.0 * f64::from(self.won) / f64::from(self.played)).round() as u32),
        }
    }
}

/// The statistics of the account over the given matches, most recent first.
pub fn of(account: Uuid, matches: &[StoredMatch]) -> PlayerStatistics {
    let mut overall = Tally::new("ALL");
    let mut by_variant = [Tally::new("CLASSIC"), Tally::new("CONTREE")];
    let mut by_difficulty = [Tally::new("RELAXED"), Tally::new("CHALLENGING")];
    let mut recent = Vec::new();
    let mut contracts = Contracts::default();
    let (mut rounds, mut points) = (0u32, 0i64);
    for stored in matches {
        let Some(position) = stored
            .seats
            .iter()
            .position(|seat| seat.account == Some(account))
        else {
            continue;
        };
        let team = stored.seats[position].team;
        let won = stored.winning_team == team;
        overall.add(won);
        let variant = variant_code(stored.variant);
        if let Some(tally) = by_variant.iter_mut().find(|tally| tally.key == variant) {
            tally.add(won);
        }
        if stored.mode == Mode::Solo
            && let Some(difficulty) = stored.difficulty.map(difficulty_code)
            && let Some(tally) = by_difficulty
                .iter_mut()
                .find(|tally| tally.key == difficulty)
        {
            tally.add(won);
        }
        if recent.len() < RECENT_MATCHES && stored.seats.len() == 4 {
            let north_south = team == Team::NorthSouth;
            let (team_score, opponent_score) = if north_south {
                (stored.north_south_score, stored.east_west_score)
            } else {
                (stored.east_west_score, stored.north_south_score)
            };
            let name = |offset: usize| stored.seats[(position + offset) % 4].name.clone();
            recent.push(RecentMatch {
                ended_at: stored.ended_at,
                mode: mode_code(stored.mode),
                variant,
                difficulty: stored.difficulty.map(difficulty_code),
                partner: name(2),
                opponents: vec![name(1), name(3)],
                team_score,
                opponent_score,
                won,
                target: stored.target,
            });
        }
        for round in &stored.rounds {
            let declared = round.declaring_team == team;
            if declared {
                contracts.taken += 1;
                contracts.made += u32::from(round.contract_made);
            }
            if round.coinched && !declared {
                contracts.coinches_by_team += 1;
                contracts.coinches_by_team_won += u32::from(!round.contract_made);
            }
            if round.coinched && declared {
                contracts.coinches_against_team += 1;
                contracts.coinches_against_team_won += u32::from(!round.contract_made);
            }
            contracts.capots += u32::from(round.capot_team == Some(team));
            contracts.belotes += u32::from(round.belote_team == Some(team));
            rounds += 1;
            points += i64::from(if team == Team::NorthSouth {
                round.north_south_points
            } else {
                round.east_west_points
            });
        }
    }
    PlayerStatistics {
        overall: overall.record(),
        by_variant: by_variant.iter().map(Tally::record).collect(),
        by_difficulty: by_difficulty.iter().map(Tally::record).collect(),
        recent_matches: recent,
        contracts,
        rounds,
        average_points_per_round: (rounds > 0)
            .then(|| (10.0 * points as f64 / f64::from(rounds)).round() / 10.0),
    }
}

#[cfg(test)]
mod tests {
    //! Ports Java's `PlayerStatisticsTest`: Ana plays East (seat 1) with Chloé (seat 3) against Ben and David.

    use beelot_game::{Difficulty, Trump, Variant};
    use time::macros::datetime;

    use super::*;
    use crate::db::matches::{StoredRound, StoredSeat};

    const ANA: Uuid = Uuid::from_u128(0xa1a);

    fn seat(account: Option<Uuid>, name: &str, team: Team, bot: bool) -> StoredSeat {
        StoredSeat {
            account,
            name: name.into(),
            team,
            bot,
        }
    }

    fn stored(
        mode: Mode,
        variant: Variant,
        difficulty: Option<Difficulty>,
        winner: Team,
        rounds: Vec<StoredRound>,
    ) -> StoredMatch {
        let east_west_won = winner == Team::EastWest;
        StoredMatch {
            id: Uuid::new_v4(),
            ended_at: datetime!(2026-10-24 12:00 UTC),
            mode,
            variant,
            difficulty,
            north_south_score: if east_west_won { 640 } else { 1010 },
            east_west_score: if east_west_won { 1020 } else { 600 },
            winning_team: winner,
            announcements: false,
            all_trumps: false,
            target: 1_000,
            seats: vec![
                seat(None, "Ben", Team::NorthSouth, false),
                seat(Some(ANA), "Ana", Team::EastWest, false),
                seat(None, "David", Team::NorthSouth, true),
                seat(None, "Chloé", Team::EastWest, false),
            ],
            rounds,
        }
    }

    fn round(
        declaring: Team,
        coinched: bool,
        made: bool,
        capot: Option<Team>,
        belote: Option<Team>,
        north_south: i32,
        east_west: i32,
    ) -> StoredRound {
        StoredRound {
            declaring_team: declaring,
            trump: Trump::Spades,
            contract_value: Some(100),
            coinched,
            contract_made: made,
            capot_team: capot,
            belote_team: belote,
            north_south_points: north_south,
            east_west_points: east_west,
            north_south_announcements: 0,
            east_west_announcements: 0,
        }
    }

    fn record(
        key: &'static str,
        played: u32,
        won: u32,
        lost: u32,
        win_rate: Option<u32>,
    ) -> Record {
        Record {
            key,
            played,
            won,
            lost,
            win_rate,
        }
    }

    #[test]
    fn counts_matches_played_won_and_lost_overall_per_variant_and_per_solo_difficulty() {
        let matches = [
            stored(
                Mode::Solo,
                Variant::Contree,
                Some(Difficulty::Challenging),
                Team::EastWest,
                vec![],
            ),
            stored(
                Mode::Solo,
                Variant::Classic,
                Some(Difficulty::Relaxed),
                Team::NorthSouth,
                vec![],
            ),
            stored(
                Mode::Private,
                Variant::Contree,
                None,
                Team::EastWest,
                vec![],
            ),
        ];
        let statistics = of(ANA, &matches);
        assert_eq!(statistics.overall, record("ALL", 3, 2, 1, Some(67)));
        assert_eq!(
            statistics.by_variant,
            [
                record("CLASSIC", 1, 0, 1, Some(0)),
                record("CONTREE", 2, 2, 0, Some(100))
            ]
        );
        assert_eq!(
            statistics.by_difficulty,
            [
                record("RELAXED", 1, 0, 1, Some(0)),
                record("CHALLENGING", 1, 1, 0, Some(100))
            ]
        );
    }

    #[test]
    fn shows_recent_matches_from_the_players_side() {
        let statistics = of(
            ANA,
            &[stored(
                Mode::Public,
                Variant::Contree,
                None,
                Team::EastWest,
                vec![],
            )],
        );
        let recent = &statistics.recent_matches[0];
        assert_eq!(recent.partner, "Chloé");
        assert_eq!(
            recent.opponents,
            ["David", "Ben"],
            "in play order after Ana"
        );
        assert_eq!(recent.team_score, 1020);
        assert_eq!(recent.opponent_score, 640);
        assert!(recent.won);
        assert_eq!(recent.mode, "PUBLIC");
    }

    #[test]
    fn keeps_the_twenty_most_recent_matches() {
        let matches: Vec<StoredMatch> = (0..25)
            .map(|_| {
                stored(
                    Mode::Solo,
                    Variant::Classic,
                    Some(Difficulty::Relaxed),
                    Team::EastWest,
                    vec![],
                )
            })
            .collect();
        let statistics = of(ANA, &matches);
        assert_eq!(statistics.overall.played, 25);
        assert_eq!(statistics.recent_matches.len(), 20);
    }

    #[test]
    fn counts_contracts_coinches_capots_belotes_and_points_for_the_players_team() {
        let rounds = vec![
            round(
                Team::EastWest,
                false,
                true,
                None,
                Some(Team::EastWest),
                0,
                120,
            ),
            round(Team::EastWest, true, false, None, None, 324, 0),
            round(
                Team::NorthSouth,
                true,
                false,
                Some(Team::EastWest),
                None,
                0,
                500,
            ),
            round(
                Team::NorthSouth,
                false,
                true,
                None,
                Some(Team::NorthSouth),
                140,
                42,
            ),
        ];
        let statistics = of(
            ANA,
            &[stored(
                Mode::Solo,
                Variant::Contree,
                Some(Difficulty::Relaxed),
                Team::EastWest,
                rounds,
            )],
        );
        assert_eq!(
            statistics.contracts,
            Contracts {
                taken: 2,
                made: 1,
                coinches_by_team: 1,
                coinches_by_team_won: 1,
                coinches_against_team: 1,
                coinches_against_team_won: 1,
                capots: 1,
                belotes: 1,
            }
        );
        assert_eq!(statistics.rounds, 4);
        assert_eq!(
            statistics.average_points_per_round,
            Some((120.0 + 0.0 + 500.0 + 42.0) / 4.0)
        );
    }

    #[test]
    fn a_player_without_matches_has_no_rates_yet() {
        let statistics = of(ANA, &[]);
        assert_eq!(statistics.overall, record("ALL", 0, 0, 0, None));
        assert_eq!(statistics.average_points_per_round, None);
    }

    #[test]
    fn the_json_keeps_javas_field_names() {
        let json = serde_json::to_value(of(ANA, &[])).unwrap();
        for field in [
            "overall",
            "byVariant",
            "byDifficulty",
            "recentMatches",
            "contracts",
            "rounds",
            "averagePointsPerRound",
        ] {
            assert!(json.get(field).is_some(), "{field}");
        }
        assert!(json["overall"].get("winRate").is_some());
        assert!(json["contracts"].get("coinchesAgainstTeamWon").is_some());
    }
}
