//! What a player can do at a table: one tagged JSON object per request.

use beelot_game::Suit;
use serde::Deserialize;

use super::reactions::TableReaction;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    Ready {
        ready: bool,
    },
    Seat {
        position: usize,
    },
    Start {
        #[serde(default)]
        fill_with_bots: bool,
    },
    Settings {
        announcements: Option<bool>,
        turn_timer: Option<u32>,
    },
    Pass,
    Trump {
        suit: Suit,
    },
    Contract {
        value: i32,
        suit: Option<Suit>,
    },
    Coinche,
    Card {
        rank: String,
        suit: Suit,
        declare: Option<bool>,
    },
    ContinueTrick,
    NextRound,
    Rematch,
    React {
        reaction: TableReaction,
    },
    Leave,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(value: serde_json::Value) -> Action {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn actions_are_tagged_by_type_with_camel_case_fields() {
        assert_eq!(
            parse(json!({"type": "ready", "ready": true})),
            Action::Ready { ready: true }
        );
        assert_eq!(
            parse(json!({"type": "start", "fillWithBots": true})),
            Action::Start {
                fill_with_bots: true
            }
        );
        assert_eq!(
            parse(json!({"type": "settings", "turnTimer": 30})),
            Action::Settings {
                announcements: None,
                turn_timer: Some(30)
            }
        );
        assert_eq!(
            parse(json!({"type": "contract", "value": 90, "suit": "HEARTS"})),
            Action::Contract {
                value: 90,
                suit: Some(Suit::Hearts)
            }
        );
        assert_eq!(
            parse(json!({"type": "card", "rank": "10", "suit": "SPADES", "declare": true})),
            Action::Card {
                rank: "10".into(),
                suit: Suit::Spades,
                declare: Some(true)
            }
        );
        assert_eq!(
            parse(json!({"type": "continueTrick"})),
            Action::ContinueTrick
        );
        assert_eq!(parse(json!({"type": "nextRound"})), Action::NextRound);
        assert_eq!(
            parse(json!({"type": "react", "reaction": "WELL_PLAYED"})),
            Action::React {
                reaction: TableReaction::WellPlayed
            }
        );
    }

    #[test]
    fn unknown_actions_are_refused() {
        assert!(serde_json::from_value::<Action>(json!({"type": "cheat"})).is_err());
        assert!(serde_json::from_value::<Action>(json!({"type": "ready"})).is_err());
    }
}
