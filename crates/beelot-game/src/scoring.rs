//! The score of a round once its tricks are played, FFB-style (US-060). A team's total is its
//! card points, dix de der, Belote and announcements, and the taker's total decides the contract:
//! in Classic it must beat the defence's total, in Contrée it must reach the bid.

use serde::{Deserialize, Deserializer, Serialize};

use crate::announcements::Announcement;
use crate::card::{Seat, Team, Trump, Variant};

/// A player's declared announcements, and whether their team scores them. The team is
/// `Team::of(seat)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Declaration {
    pub seat: Seat,
    pub announcements: Vec<Announcement>,
    pub scored: bool,
}

/// `capotTeam` is the empty string (or null) when nobody took every trick.
fn capot_team<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Team>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Team(Team),
        Text(String),
    }
    match Option::<Raw>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Raw::Team(team)) => Ok(Some(team)),
        Some(Raw::Text(text)) if text.is_empty() => Ok(None),
        Some(Raw::Text(text)) => Err(serde::de::Error::custom(format!("unknown team: {text}"))),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub north_south_card_points: i32,
    pub east_west_card_points: i32,
    pub north_south_dix_de_der: i32,
    pub east_west_dix_de_der: i32,
    pub north_south_belote_bonus: i32,
    pub east_west_belote_bonus: i32,
    pub contract_made: bool,
    pub north_south_awarded: i32,
    pub east_west_awarded: i32,
    #[serde(deserialize_with = "capot_team", default)]
    pub capot_team: Option<Team>,
    pub north_south_announcements: i32,
    pub east_west_announcements: i32,
    pub announcements: Vec<Declaration>,
}

/// A finished round: who declared, the trump, the Contrée contract (None in classic Belote), and
/// the result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundSummary {
    pub declaring_team: Team,
    pub trump: Trump,
    pub contract_value: Option<i32>,
    pub coinched: bool,
    pub result: RoundResult,
}

/// A round's totals per team; the announcements are those that score, so one side is 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub variant: Variant,
    pub north_south_declares: bool,
    pub contract_value: i32,
    pub coinched: bool,
    pub north_south_card_points: i32,
    pub east_west_card_points: i32,
    pub north_south_dix_de_der: i32,
    pub east_west_dix_de_der: i32,
    pub north_south_belote: i32,
    pub east_west_belote: i32,
    pub north_south_tricks: i32,
    pub north_south_announcements: i32,
    pub east_west_announcements: i32,
}

/// A failed contract gives the defence 162 and every announcement that scored, and leaves the
/// taker its Belote. A taker taking every trick scores 250, plus the bid in Contrée, and its own
/// announcements; a capoted taker fails and the defence scores 250 and every announcement that
/// scored. Belote always stays with its team, and a coinche doubles both scores.
pub fn score(totals: &Totals, declarations: &[Declaration]) -> RoundResult {
    let t = totals;
    let ns_total = t
        .north_south_card_points
        .wrapping_add(t.north_south_dix_de_der)
        .wrapping_add(t.north_south_belote)
        .wrapping_add(t.north_south_announcements);
    let ew_total = t
        .east_west_card_points
        .wrapping_add(t.east_west_dix_de_der)
        .wrapping_add(t.east_west_belote)
        .wrapping_add(t.east_west_announcements);
    let ns_declares = t.north_south_declares;
    let taker_total = if ns_declares { ns_total } else { ew_total };
    let defence_total = if ns_declares { ew_total } else { ns_total };
    let capot_team = match t.north_south_tricks {
        8 => Some(Team::NorthSouth),
        0 => Some(Team::EastWest),
        _ => None,
    };
    let declaring_team = if ns_declares {
        Team::NorthSouth
    } else {
        Team::EastWest
    };
    let contract_made = match capot_team {
        None if t.variant == Variant::Contree => taker_total >= t.contract_value,
        None => taker_total > defence_total,
        Some(capot) => capot == declaring_team,
    };

    let all_announcements = t
        .north_south_announcements
        .wrapping_add(t.east_west_announcements);
    let (mut ns_awarded, mut ew_awarded);
    if let Some(capot) = capot_team {
        let ns_capot = capot == Team::NorthSouth;
        let taker_capot = capot == declaring_team;
        let bonus = if taker_capot {
            if t.variant == Variant::Contree {
                t.contract_value
            } else {
                0
            }
        } else {
            all_announcements
        };
        let capot_score = 250i32.wrapping_add(bonus);
        ns_awarded = if ns_capot {
            capot_score
                .wrapping_add(if taker_capot {
                    t.north_south_announcements
                } else {
                    0
                })
                .wrapping_add(t.north_south_belote)
        } else {
            t.north_south_belote
        };
        ew_awarded = if ns_capot {
            t.east_west_belote
        } else {
            capot_score
                .wrapping_add(if taker_capot {
                    t.east_west_announcements
                } else {
                    0
                })
                .wrapping_add(t.east_west_belote)
        };
    } else if contract_made {
        ns_awarded = ns_total;
        ew_awarded = ew_total;
    } else {
        let defence = 162i32.wrapping_add(all_announcements);
        ns_awarded = if ns_declares {
            t.north_south_belote
        } else {
            defence.wrapping_add(t.north_south_belote)
        };
        ew_awarded = if ns_declares {
            defence.wrapping_add(t.east_west_belote)
        } else {
            t.east_west_belote
        };
    }
    if t.coinched {
        ns_awarded = ns_awarded.wrapping_mul(2);
        ew_awarded = ew_awarded.wrapping_mul(2);
    }
    RoundResult {
        north_south_card_points: t.north_south_card_points,
        east_west_card_points: t.east_west_card_points,
        north_south_dix_de_der: t.north_south_dix_de_der,
        east_west_dix_de_der: t.east_west_dix_de_der,
        north_south_belote_bonus: t.north_south_belote,
        east_west_belote_bonus: t.east_west_belote,
        contract_made,
        north_south_awarded: ns_awarded,
        east_west_awarded: ew_awarded,
        capot_team,
        north_south_announcements: t.north_south_announcements,
        east_west_announcements: t.east_west_announcements,
        announcements: declarations.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// North–South take the contract.
    #[allow(clippy::too_many_arguments)]
    fn classic(
        ns_cards: i32,
        ew_cards: i32,
        ns_dix: i32,
        ew_dix: i32,
        ns_belote: i32,
        ew_belote: i32,
        ns_tricks: i32,
        ns_ann: i32,
        ew_ann: i32,
    ) -> Totals {
        Totals {
            variant: Variant::Classic,
            north_south_declares: true,
            contract_value: 82,
            coinched: false,
            north_south_card_points: ns_cards,
            east_west_card_points: ew_cards,
            north_south_dix_de_der: ns_dix,
            east_west_dix_de_der: ew_dix,
            north_south_belote: ns_belote,
            east_west_belote: ew_belote,
            north_south_tricks: ns_tricks,
            north_south_announcements: ns_ann,
            east_west_announcements: ew_ann,
        }
    }

    /// North–South bid the contract.
    #[allow(clippy::too_many_arguments)]
    fn contree(
        bid: i32,
        coinched: bool,
        ns_cards: i32,
        ew_cards: i32,
        ns_dix: i32,
        ew_dix: i32,
        ns_belote: i32,
        ew_belote: i32,
        ns_tricks: i32,
        ns_ann: i32,
        ew_ann: i32,
    ) -> Totals {
        Totals {
            variant: Variant::Contree,
            contract_value: bid,
            coinched,
            ..classic(
                ns_cards, ew_cards, ns_dix, ew_dix, ns_belote, ew_belote, ns_tricks, ns_ann, ew_ann,
            )
        }
    }

    fn run(totals: Totals) -> RoundResult {
        score(&totals, &[])
    }

    #[test]
    fn classic_belote_counts_towards_the_contract() {
        let result = run(classic(65, 87, 10, 0, 20, 0, 5, 0, 0));
        assert!(result.contract_made);
        assert_eq!(result.north_south_awarded, 95);
        assert_eq!(result.east_west_awarded, 87);
    }

    #[test]
    fn classic_tie_fails() {
        let result = run(classic(61, 91, 10, 0, 20, 0, 4, 0, 0));
        assert!(!result.contract_made);
        assert_eq!(result.north_south_awarded, 20);
        assert_eq!(result.east_west_awarded, 162);
    }

    #[test]
    fn contree_belote_counts_towards_the_bid() {
        let result = run(contree(100, false, 72, 80, 10, 0, 20, 0, 5, 0, 0));
        assert!(result.contract_made);
        assert_eq!(result.north_south_awarded, 102);
        assert_eq!(result.east_west_awarded, 80);
    }

    #[test]
    fn announcements_count_towards_the_contract() {
        let result = run(classic(66, 86, 10, 0, 0, 0, 5, 20, 0));
        assert!(result.contract_made);
        assert_eq!(result.north_south_awarded, 96);
        assert_eq!(result.east_west_awarded, 86);
        assert_eq!(result.north_south_announcements, 20);
        assert_eq!(result.east_west_announcements, 0);
    }

    #[test]
    fn failed_contract_gives_the_defence_every_scored_announcement() {
        let result = run(classic(40, 112, 0, 10, 0, 0, 2, 50, 0));
        assert!(!result.contract_made);
        assert_eq!(result.north_south_awarded, 0);
        assert_eq!(result.east_west_awarded, 212);
    }

    #[test]
    fn capot_adds_the_teams_own_announcements() {
        let result = run(classic(152, 0, 10, 0, 0, 20, 8, 50, 0));
        assert_eq!(result.capot_team, Some(Team::NorthSouth));
        assert_eq!(result.north_south_awarded, 300);
        assert_eq!(result.east_west_awarded, 20);
    }

    #[test]
    fn capoted_taker_fails_and_loses_its_announcements_to_the_defence() {
        let result = run(contree(80, false, 0, 152, 0, 10, 0, 0, 0, 200, 0));
        assert!(!result.contract_made);
        assert_eq!(result.capot_team, Some(Team::EastWest));
        assert_eq!(result.north_south_awarded, 0);
        assert_eq!(result.east_west_awarded, 450);
    }

    #[test]
    fn coinche_doubles_both_scores() {
        let result = run(contree(90, true, 66, 86, 10, 0, 0, 0, 5, 20, 0));
        assert!(result.contract_made);
        assert_eq!(result.north_south_awarded, 192);
        assert_eq!(result.east_west_awarded, 172);
    }

    #[test]
    fn without_announcements_card_points_decide() {
        assert!(run(classic(82, 70, 0, 10, 0, 0, 5, 0, 0)).contract_made);
        assert!(!run(classic(81, 71, 0, 10, 0, 0, 4, 0, 0)).contract_made);
    }

    #[test]
    fn round_result_reads_the_java_json() {
        let json = r#"{"northSouthCardPoints":53,"eastWestCardPoints":99,"northSouthDixDeDer":10,"eastWestDixDeDer":0,"northSouthBeloteBonus":20,"eastWestBeloteBonus":0,"contractMade":true,"northSouthAwarded":83,"eastWestAwarded":99,"capotTeam":"","northSouthAnnouncements":20,"eastWestAnnouncements":0,"announcements":[{"seat":2,"player":"South","team":"North–South","announcements":[{"kind":"TIERCE","cards":["10H","JH","QH"],"value":20,"suit":"HEARTS"}],"scored":true}]}"#;
        let result: RoundResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.capot_team, None);
        assert_eq!(result.announcements[0].seat, 2);
        assert!(result.announcements[0].scored);
        let capot = json.replace(r#""capotTeam":"""#, r#""capotTeam":"North–South""#);
        let result: RoundResult = serde_json::from_str(&capot).unwrap();
        assert_eq!(result.capot_team, Some(Team::NorthSouth));
    }
}
