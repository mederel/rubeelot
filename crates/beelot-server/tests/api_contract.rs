//! The snapshot contract: example snapshots, serialized, must match `tests/contract/*.json`. Run with
//! `UPDATE_CONTRACT=1` to rewrite the files after an intended change, and review the diff. Also ports Java's
//! `PlayerViewsTest` and checks the English messages against Java's.

use std::path::PathBuf;
use std::time::Duration;

use beelot_game::bot::strategies;
use beelot_game::{Difficulty, Suit, Variant};
use beelot_server::api::{Viewer, snapshot};
use beelot_server::tables::{Action, Actor, Lobby, Table, TableKind, TableSetup, local_seats};
use serde_json::Value;
use time::OffsetDateTime;
use time::macros::datetime;
use uuid::Uuid;

const T0: OffsetDateTime = datetime!(2026-10-24 12:00 UTC);
const TABLE_ID: Uuid = Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);

fn setup(seed: i64) -> TableSetup {
    TableSetup::seeded(seed).with_strategy(strategies::for_difficulty(Difficulty::Relaxed))
}

fn solo(variant: Variant, seed: i64) -> Table {
    Table::local(
        TABLE_ID,
        Difficulty::Relaxed,
        Lobby::local(local_seats(None).unwrap(), variant, true, 1_000),
        setup(seed),
        T0,
    )
}

fn private(variant: Variant, seed: i64) -> Table {
    let lobby = Lobby::private("Ana".into(), None, variant);
    Table::online(
        TABLE_ID,
        TableKind::Private,
        Some("ABC234".into()),
        lobby,
        setup(seed),
        T0,
    )
}

fn act(table: &mut Table, actor: Actor, action: Action) {
    table.apply(actor, action, T0).expect("an accepted action");
}

fn card(table: &Table, seat: usize) -> Action {
    let card = table.board().unwrap().legal_cards(seat)[0];
    Action::Card {
        rank: card.rank.label().into(),
        suit: card.suit,
        declare: Some(true),
    }
}

/// The serialized snapshot, with the random auction id replaced so the files stay stable.
fn json(table: &Table, viewer: Viewer) -> Value {
    let mut value = serde_json::to_value(snapshot(table, viewer, T0)).unwrap();
    if let Some(bidding) = value.get_mut("bidding") {
        bidding["auctionId"] = "<auction-id>".into();
    }
    value
}

fn check(scenario: &str, value: Value) {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "contract",
        &format!("{scenario}.json"),
    ]
    .iter()
    .collect();
    let text = serde_json::to_string_pretty(&value).unwrap() + "\n";
    if std::env::var_os("UPDATE_CONTRACT").is_some() {
        std::fs::write(&path, text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is missing: run with UPDATE_CONTRACT=1", path.display()));
    assert_eq!(
        text, expected,
        "the {scenario} snapshot changed: run with UPDATE_CONTRACT=1 and review the diff"
    );
}

#[test]
fn lobby_waiting() {
    let mut table = private(Variant::Classic, 1);
    let ben = table.join("Ben".into(), None, T0).unwrap();
    act(&mut table, Actor::Seat(0), Action::Ready { ready: true });
    let value = json(&table, Viewer::Seat(ben));
    assert_eq!(value["you"], 1);
    assert_eq!(value["table"]["ownerPosition"], 0);
    assert_eq!(value["table"]["seats"][1]["you"], true);
    assert!(value.get("bidding").is_none());
    check("lobby_waiting", value);
}

/// A solo Classic table where everyone passed in the first round, so the human may choose another suit.
#[test]
fn classic_auction_round_2() {
    let table = (0..)
        .map(|seed| {
            let mut table = solo(Variant::Classic, seed);
            act(&mut table, Actor::Device, Action::Pass);
            table
        })
        .find(|table| {
            table.board().is_none()
                && table.bidding().unwrap().round() == 2
                && table.bidding().unwrap().active_seat() == 0
        })
        .unwrap();
    let value = json(&table, Viewer::Device);
    let message = value["bidding"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Everyone passed. Choose any trump suit except "),
        "{message}"
    );
    check("classic_auction_round_2", value);
}

#[test]
fn contree_bidding() {
    let table = (0..)
        .map(|seed| {
            let mut table = solo(Variant::Contree, seed);
            act(
                &mut table,
                Actor::Device,
                Action::Contract {
                    value: 90,
                    suit: Some(Suit::Hearts),
                },
            );
            table
        })
        .find(|table| table.board().is_none() && table.bidding().unwrap().auction().len() > 1)
        .unwrap();
    let value = json(&table, Viewer::Device);
    assert_eq!(value["bidding"]["auction"][0]["call"], "90 Hearts");
    assert_eq!(value["bidding"]["calls"][0]["call"], "90 Hearts");
    check("contree_bidding", value);
}

#[test]
fn trick_under_review() {
    let mut table = private(Variant::Contree, 4);
    act(&mut table, Actor::Seat(0), Action::Ready { ready: true });
    act(
        &mut table,
        Actor::Seat(0),
        Action::Start {
            fill_with_bots: true,
        },
    );
    while table.board().is_none() {
        let action = if table.bidding().unwrap().highest_bid() < 80 {
            Action::Contract {
                value: 80,
                suit: Some(Suit::Spades),
            }
        } else {
            Action::Pass
        };
        act(&mut table, Actor::Seat(0), action);
    }
    while !table.board().unwrap().reviewing_completed_trick() {
        let action = card(&table, 0);
        act(&mut table, Actor::Seat(0), action);
    }
    let value = json(&table, Viewer::Seat(0));
    assert_eq!(value["board"]["reviewingCompletedTrick"], true);
    assert_ne!(value["board"]["trickWinner"], "");
    assert_eq!(value["board"]["currentTrick"].as_array().unwrap().len(), 4);
    check("trick_under_review", value);
}

/// A finished round in which someone's announcements were revealed.
#[test]
fn round_over_with_announcements() {
    let table = (0..)
        .map(|seed| {
            let mut table = solo(Variant::Contree, seed);
            while table.board().is_none() {
                let action = if table.bidding().unwrap().highest_bid() < 80 {
                    Action::Contract {
                        value: 80,
                        suit: Some(Suit::Spades),
                    }
                } else {
                    Action::Pass
                };
                act(&mut table, Actor::Device, action);
            }
            while table.board().unwrap().round_result().is_none() {
                let action = if table.board().unwrap().reviewing_completed_trick() {
                    Action::ContinueTrick
                } else {
                    card(&table, 0)
                };
                act(&mut table, Actor::Device, action);
            }
            table
        })
        .find(|table| {
            !table
                .board()
                .unwrap()
                .round_result()
                .unwrap()
                .announcements
                .is_empty()
        })
        .unwrap();
    let value = json(&table, Viewer::Device);
    let declaration = &value["board"]["roundResult"]["announcements"][0];
    assert!(declaration["player"].is_string());
    assert!(declaration["team"].is_string());
    assert!(declaration["announcements"][0]["topCard"]["symbol"].is_string());
    check("round_over_with_announcements", value);
}

#[test]
fn messages_match_java() {
    let mut table = private(Variant::Contree, 5);
    for name in ["Ben", "Chloe", "David"] {
        table.join(name.into(), None, T0).unwrap();
    }
    for seat in 0..4 {
        act(&mut table, Actor::Seat(seat), Action::Ready { ready: true });
    }
    act(
        &mut table,
        Actor::Seat(0),
        Action::Start {
            fill_with_bots: false,
        },
    );
    let message = |table: &Table| json(table, Viewer::Seat(0))["bidding"]["message"].clone();
    assert_eq!(
        message(&table),
        "Eight cards have been dealt. Bid from 80 to 160 or pass."
    );
    act(&mut table, Actor::Seat(0), Action::Pass);
    assert_eq!(message(&table), "Ben is deciding.");
    act(
        &mut table,
        Actor::Seat(1),
        Action::Contract {
            value: 90,
            suit: Some(Suit::Hearts),
        },
    );
    assert_eq!(message(&table), "Ben bids 90 Hearts.");
    act(&mut table, Actor::Seat(2), Action::Coinche);
    assert_eq!(message(&table), "Chloe coinches the contract.");
    let calls = json(&table, Viewer::Seat(0))["bidding"]["calls"].clone();
    let calls: Vec<&str> = (0..4)
        .map(|seat| calls[seat]["call"].as_str().unwrap())
        .collect();
    assert_eq!(calls, ["Pass", "90 Hearts", "Coinche!", ""]);

    // Everyone passes: the cards are dealt again.
    let mut redeal = private(Variant::Contree, 6);
    for name in ["Ben", "Chloe", "David"] {
        redeal.join(name.into(), None, T0).unwrap();
    }
    for seat in 0..4 {
        act(
            &mut redeal,
            Actor::Seat(seat),
            Action::Ready { ready: true },
        );
    }
    act(
        &mut redeal,
        Actor::Seat(0),
        Action::Start {
            fill_with_bots: false,
        },
    );
    for seat in 0..4 {
        act(&mut redeal, Actor::Seat(seat), Action::Pass);
    }
    assert_eq!(
        message(&redeal),
        "Everyone passed. The cards have been redealt."
    );
    act(
        &mut redeal,
        Actor::Seat(1),
        Action::Contract {
            value: 80,
            suit: Some(Suit::Clubs),
        },
    );
    for seat in [2, 3, 0] {
        act(&mut redeal, Actor::Seat(seat), Action::Pass);
    }
    assert_eq!(message(&redeal), "The 80 Clubs contract is accepted.");
}

#[test]
fn belote_and_rebelote_are_announced() {
    for seed in 0.. {
        let mut table = solo(Variant::Contree, seed);
        while table.board().is_none() {
            let action = if table.bidding().unwrap().highest_bid() < 80 {
                Action::Contract {
                    value: 80,
                    suit: Some(Suit::Spades),
                }
            } else {
                Action::Pass
            };
            act(&mut table, Actor::Device, action);
        }
        let Some(seat) = table.board().unwrap().belote_seat() else {
            continue;
        };
        let name = table.lobby().seat(seat).unwrap().name.clone();
        let mut messages = Vec::new();
        while table.board().unwrap().round_result().is_none() {
            let message = json(&table, Viewer::Device)["board"]["declarationMessage"].clone();
            if messages.last() != Some(&message) {
                messages.push(message);
            }
            let action = if table.board().unwrap().reviewing_completed_trick() {
                Action::ContinueTrick
            } else {
                card(&table, 0)
            };
            act(&mut table, Actor::Device, action);
        }
        let last = json(&table, Viewer::Device)["board"]["declarationMessage"].clone();
        if messages.last() != Some(&last) {
            messages.push(last);
        }
        assert_eq!(
            messages[0], "",
            "nothing is announced before the first card of the pair"
        );
        assert!(
            messages.contains(&Value::from(format!("{name} declares Belote."))),
            "{messages:?}"
        );
        assert!(
            messages.contains(&Value::from(format!(
                "{name} declares Rebelote: 20 bonus points."
            ))),
            "{messages:?}"
        );
        return;
    }
}

// PlayerViewsTest: the views show only what their player can see.

#[test]
fn every_player_sees_which_seat_took_the_contract() {
    let mut table = private(Variant::Contree, 7);
    for name in ["Ben", "Chloe", "David"] {
        table.join(name.into(), None, T0).unwrap();
    }
    for seat in 0..4 {
        act(&mut table, Actor::Seat(seat), Action::Ready { ready: true });
    }
    act(
        &mut table,
        Actor::Seat(0),
        Action::Start {
            fill_with_bots: false,
        },
    );
    act(&mut table, Actor::Seat(0), Action::Pass);
    act(&mut table, Actor::Seat(1), Action::Pass);
    act(&mut table, Actor::Seat(2), Action::Pass);
    act(
        &mut table,
        Actor::Seat(3),
        Action::Contract {
            value: 80,
            suit: Some(Suit::Hearts),
        },
    );
    for seat in 0..3 {
        act(&mut table, Actor::Seat(seat), Action::Pass);
    }
    for seat in 0..4 {
        let value = json(&table, Viewer::Seat(seat));
        assert_eq!(value["board"]["declaringPlayerIndex"], 3);
        assert_eq!(value["board"]["currentPlayerIndex"], seat);
        let hand = value["board"]["hand"].as_array().unwrap();
        assert_eq!(hand.len(), 8, "each player sees their own hand");
        let legal = value["board"]["legalCards"].as_array().unwrap();
        assert_eq!(
            legal.is_empty(),
            seat != 0,
            "only the player to lead has legal cards"
        );
    }
}

#[test]
fn hands_are_shown_only_to_their_player() {
    let mut table = private(Variant::Contree, 8);
    for name in ["Ben", "Chloe", "David"] {
        table.join(name.into(), None, T0).unwrap();
    }
    for seat in 0..4 {
        act(&mut table, Actor::Seat(seat), Action::Ready { ready: true });
    }
    act(
        &mut table,
        Actor::Seat(0),
        Action::Start {
            fill_with_bots: false,
        },
    );
    let hands: Vec<Value> = (0..4)
        .map(|seat| json(&table, Viewer::Seat(seat))["bidding"]["hand"].clone())
        .collect();
    for (seat, hand) in hands.iter().enumerate() {
        let expected = table.bidding().unwrap().hand(seat).len();
        assert_eq!(hand.as_array().unwrap().len(), expected);
        for other in hands.iter().take(seat) {
            assert_ne!(hand, other);
        }
    }
    let text = serde_json::to_string(&snapshot(&table, Viewer::Seat(1), T0)).unwrap();
    for card in table.bidding().unwrap().hand(0) {
        let shown = format!(
            "{{\"rank\":\"{}\",\"suit\":\"{:?}\"",
            card.rank.label(),
            card.suit
        )
        .to_uppercase();
        assert!(
            !text.to_uppercase().contains(&shown),
            "Ben does not see Ana's {card}"
        );
    }
}

#[test]
fn bot_fill_deadline_is_shown_while_a_public_table_waits() {
    let lobby = Lobby::public(
        "Ana".into(),
        None,
        Variant::Classic,
        T0 + Duration::from_secs(60),
    );
    let mut table = Table::online(TABLE_ID, TableKind::Public, None, lobby, setup(9), T0);
    let value = json(&table, Viewer::Seat(0));
    assert_eq!(value["table"]["botFillAt"], "2026-10-24T12:01:00Z");
    assert_eq!(value["table"]["publicTable"], true);
    assert!(table.fill_with_bots_if_due(T0 + Duration::from_secs(60)));
    assert!(json(&table, Viewer::Seat(0))["table"]["botFillAt"].is_null());
}
