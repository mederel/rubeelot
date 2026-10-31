//! The table API, porting Java's `BotGameControllerTest`, `PrivateTableControllerTest` and
//! `MatchmakingControllerTest` to `/api/tables`.

mod common;

use axum::Router;
use axum::http::StatusCode;
use beelot_server::app::{AppState, router};
use common::{call, get_request, json_request, test_state};
use serde_json::{Value, json};

struct Player {
    table: String,
    token: String,
}

fn app() -> (Router, AppState) {
    let state = test_state();
    (router(state.clone()), state)
}

async fn create(app: &Router, body: Value) -> Player {
    let (status, created) = call(app, json_request("POST", "/api/tables", None, body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Player {
        table: created["tableId"].as_str().unwrap().into(),
        token: created["playerToken"].as_str().unwrap().into(),
    }
}

async fn solo(app: &Router, variant: &str) -> Player {
    create(
        app,
        json!({"kind": "LOCAL", "variant": variant, "difficulty": "RELAXED"}),
    )
    .await
}

async fn snapshot(app: &Router, player: &Player) -> Value {
    let uri = format!("/api/tables/{}", player.table);
    let (status, body) = call(app, get_request(&uri, Some(&player.token))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn act(app: &Router, player: &Player, action: Value) -> (StatusCode, Value) {
    let uri = format!("/api/tables/{}/actions", player.table);
    call(app, json_request("POST", &uri, Some(&player.token), action)).await
}

async fn accepted(app: &Router, player: &Player, action: Value) -> Value {
    let (status, body) = act(app, player, action.clone()).await;
    assert_eq!(status, StatusCode::OK, "{action} → {body}");
    body
}

async fn join(app: &Router, code: &str, name: &str) -> Player {
    let body = json!({"code": code, "playerName": name});
    let (status, joined) = call(app, json_request("POST", "/api/tables/join", None, body)).await;
    assert_eq!(status, StatusCode::CREATED, "{joined}");
    Player {
        table: joined["tableId"].as_str().unwrap().into(),
        token: joined["playerToken"].as_str().unwrap().into(),
    }
}

async fn private(app: &Router, name: &str, variant: &str) -> (Player, String) {
    let body = json!({"kind": "PRIVATE", "variant": variant, "playerName": name});
    let (status, created) = call(app, json_request("POST", "/api/tables", None, body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let player = Player {
        table: created["tableId"].as_str().unwrap().into(),
        token: created["playerToken"].as_str().unwrap().into(),
    };
    (player, created["code"].as_str().unwrap().into())
}

// BotGameControllerTest

#[tokio::test]
async fn creates_a_pass_and_play_table_with_bots_in_the_empty_seats() {
    let (app, _) = app();
    let player = create(
        &app,
        json!({"kind": "LOCAL", "difficulty": "RELAXED", "seats": ["Ana", null, "Ben", ""]}),
    )
    .await;
    let table = &snapshot(&app, &player).await["table"];
    assert_eq!(table["passAndPlay"], true);
    assert_eq!(table["seats"][0]["name"], "Ana");
    assert_eq!(table["seats"][0]["type"], "HUMAN");
    assert_eq!(table["seats"][1]["type"], "BOT");
    assert_eq!(table["seats"][2]["name"], "Ben");
    assert_eq!(table["seats"][3]["type"], "BOT");
}

#[tokio::test]
async fn rejects_pass_and_play_with_a_single_player() {
    let (app, _) = app();
    let body =
        json!({"kind": "LOCAL", "difficulty": "RELAXED", "seats": ["Ana", null, null, null]});
    let (status, error) = call(&app, json_request("POST", "/api/tables", None, body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["message"],
        "Pass and play needs at least two players."
    );
    assert_eq!(error["error"], "two-players");
}

#[tokio::test]
async fn creates_a_four_seat_table_at_the_selected_difficulty() {
    let (app, _) = app();
    let player = create(&app, json!({"kind": "LOCAL", "difficulty": "CHALLENGING"})).await;
    let snapshot = snapshot(&app, &player).await;
    let table = &snapshot["table"];
    assert_eq!(snapshot["kind"], "LOCAL");
    assert_eq!(table["difficulty"], "CHALLENGING");
    assert_eq!(table["difficultyLabel"], "Challenging");
    assert_eq!(table["seats"].as_array().unwrap().len(), 4);
    assert_eq!(table["seats"][0]["type"], "HUMAN");
    assert_eq!(table["seats"][0]["name"], "You");
    assert_eq!(table["variant"], "CLASSIC", "Classic unless chosen");
    assert_eq!(
        table["announcements"], true,
        "announcements unless turned off"
    );
}

#[tokio::test]
async fn starts_a_contree_board_from_an_ascending_contract_bid() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let first = snapshot(&app, &player).await;
    assert_eq!(first["table"]["variantLabel"], "Contrée");
    assert_eq!(first["bidding"]["variant"], "CONTREE");
    assert_eq!(first["bidding"]["hand"].as_array().unwrap().len(), 8);
    assert!(first["bidding"]["upturnedCard"].is_null());

    let mut after = accepted(
        &app,
        &player,
        json!({"type": "contract", "value": 160, "suit": "SPADES"}),
    )
    .await;
    while after["bidding"]["complete"] != true {
        after = accepted(&app, &player, json!({"type": "pass"})).await;
    }
    assert_eq!(after["bidding"]["highestBid"], 160);
    let board = &after["board"];
    assert_eq!(board["variant"], "CONTREE");
    assert_eq!(board["contractValue"], 160);
    assert_eq!(board["trump"], "Spades");
    assert_eq!(board["currentPlayer"], "You");
    assert_eq!(board["currentPlayerIndex"], 0);
}

#[tokio::test]
async fn rejects_a_contract_without_a_trump_suit() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let (status, error) = act(&app, &player, json!({"type": "contract", "value": 90})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "Choose a trump suit for the contract.");
}

#[tokio::test]
async fn announcements_can_be_turned_off() {
    let (app, _) = app();
    let player = create(
        &app,
        json!({"kind": "LOCAL", "variant": "CONTREE", "difficulty": "RELAXED", "announcements": false}),
    )
    .await;
    let mut view = accepted(
        &app,
        &player,
        json!({"type": "contract", "value": 160, "suit": "SPADES"}),
    )
    .await;
    while view["bidding"]["complete"] != true {
        view = accepted(&app, &player, json!({"type": "pass"})).await;
    }
    assert_eq!(view["table"]["announcements"], false);
    assert_eq!(view["board"]["announcements"], false);
    assert_eq!(view["board"]["ownAnnouncements"], json!([]));
}

#[tokio::test]
async fn a_local_table_plays_to_the_chosen_target_or_to_one_thousand() {
    let (app, _) = app();
    let default = solo(&app, "CLASSIC").await;
    let view = snapshot(&app, &default).await;
    assert_eq!(view["table"]["target"], 1_000);
    assert_eq!(view["match"]["target"], 1_000);
    let chosen = create(
        &app,
        json!({"kind": "LOCAL", "variant": "CLASSIC", "difficulty": "RELAXED", "target": 1500}),
    )
    .await;
    let view = snapshot(&app, &chosen).await;
    assert_eq!(view["table"]["target"], 1_500);
    assert_eq!(view["match"]["target"], 1_500);
}

#[tokio::test]
async fn a_contree_table_bids_and_plays_all_trumps() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    assert_eq!(snapshot(&app, &player).await["table"]["allTrumps"], true);
    // No bot can overcall 160, so the contract stands.
    let mut view = accepted(
        &app,
        &player,
        json!({"type": "contract", "value": 160, "suit": "ALL_TRUMPS"}),
    )
    .await;
    assert_eq!(view["bidding"]["highestBidSuit"], "ALL_TRUMPS");
    while view["bidding"]["complete"] != true {
        view = accepted(&app, &player, json!({"type": "pass"})).await;
    }
    assert_eq!(view["board"]["trump"], "All trumps");
    assert_eq!(view["board"]["contractValue"], 160);
}

#[tokio::test]
async fn a_table_without_all_trumps_refuses_them() {
    let (app, _) = app();
    let player = create(
        &app,
        json!({"kind": "LOCAL", "variant": "CONTREE", "difficulty": "RELAXED", "allTrumps": false}),
    )
    .await;
    assert_eq!(snapshot(&app, &player).await["table"]["allTrumps"], false);
    let (status, error) = act(
        &app,
        &player,
        json!({"type": "contract", "value": 100, "suit": "NO_TRUMPS"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["message"],
        "Tout-atout and sans-atout are not played at this table."
    );
}

#[tokio::test]
async fn the_owner_turns_all_trumps_off() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CONTREE").await;
    let guest = join(&app, &code, "Benoit").await;
    let off = accepted(
        &app,
        &owner,
        json!({"type": "settings", "allTrumps": false}),
    )
    .await;
    assert_eq!(off["table"]["allTrumps"], false);
    let (status, error) = act(&app, &guest, json!({"type": "settings", "allTrumps": true})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["message"],
        "Only the table owner can change tout-atout and sans-atout."
    );
}

#[tokio::test]
async fn a_target_that_is_not_offered_is_refused() {
    let (app, _) = app();
    let (status, _) = call(
        &app,
        json_request(
            "POST",
            "/api/tables",
            None,
            json!({"kind": "LOCAL", "variant": "CLASSIC", "target": 750}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (owner, _) = private(&app, "Ana", "CLASSIC").await;
    let (status, error) = act(&app, &owner, json!({"type": "settings", "target": 750})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["message"],
        "The match target must be 500, 1,000, 1,500 or 2,000 points."
    );
    let set = accepted(&app, &owner, json!({"type": "settings", "target": 500})).await;
    assert_eq!(set["table"]["target"], 500);
}

#[tokio::test]
async fn hints_are_offered_at_local_tables_only() {
    let (app, _) = app();
    let local = solo(&app, "CONTREE").await;
    assert_eq!(snapshot(&app, &local).await["table"]["hints"], json!(true));

    let (online, _) = private(&app, "Ana", "CONTREE").await;
    assert_eq!(
        snapshot(&app, &online).await["table"]["hints"],
        json!(false)
    );
}

async fn hint(app: &Router, player: &Player) -> (StatusCode, Value) {
    let uri = format!("/api/tables/{}/hint", player.table);
    call(app, get_request(&uri, Some(&player.token))).await
}

const CARD_REASONS: [&str; 8] = [
    "LEAD_TRUMPS",
    "CASH_MASTER",
    "LEAD_LOW",
    "GIVE_POINTS",
    "KEEP_PARTNERS_TRICK",
    "WIN_CHEAPLY",
    "THROW_CHEAPEST",
    "BEST_FOR_ROUND",
];

const BID_REASONS: [&str; 5] = [
    "TAKE",
    "SUPPORT_PARTNER",
    "BID_HAND_VALUE",
    "COINCHE",
    "PASS_WEAK",
];

/// Bids 160 and passes until the cards are played, leaving the human to play.
async fn into_card_play(app: &Router, player: &Player) -> Value {
    let mut view = accepted(
        app,
        player,
        json!({"type": "contract", "value": 160, "suit": "SPADES"}),
    )
    .await;
    while view["bidding"]["complete"] != true {
        view = accepted(app, player, json!({"type": "pass"})).await;
    }
    view
}

#[tokio::test]
async fn a_solo_player_gets_a_card_hint() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let view = into_card_play(&app, &player).await;

    let (status, answer) = hint(&app, &player).await;

    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["type"], "card");
    assert!(
        view["board"]["legalCards"]
            .as_array()
            .unwrap()
            .contains(&answer["card"])
    );
    assert!(CARD_REASONS.contains(&answer["reason"].as_str().unwrap()));
}

#[tokio::test]
async fn a_solo_player_gets_a_bid_hint() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;

    let (status, answer) = hint(&app, &player).await;

    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["type"], "bid");
    assert!(BID_REASONS.contains(&answer["reason"].as_str().unwrap()));
    match answer["action"]["type"].as_str().unwrap() {
        "contract" => {
            assert!(answer["action"]["value"].is_i64());
            assert!(answer["action"]["suit"].is_string());
        }
        "pass" | "coinche" => {}
        other => panic!("a Contrée hint cannot be a {other}"),
    }
}

#[tokio::test]
async fn a_classic_hint_is_a_pass_or_a_trump_with_its_suit() {
    let (app, _) = app();
    let player = solo(&app, "CLASSIC").await;

    let (status, answer) = hint(&app, &player).await;

    assert_eq!(status, StatusCode::OK, "{answer}");
    match answer["action"]["type"].as_str().unwrap() {
        "trump" => {
            assert!(answer["action"]["suit"].is_string(), "{answer}");
            assert_eq!(answer["reason"], "TAKE");
        }
        "pass" => assert_eq!(answer["reason"], "PASS_WEAK"),
        other => panic!("a Classic hint cannot be a {other}"),
    }
}

#[tokio::test]
async fn asking_for_a_hint_changes_nothing() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let before = snapshot(&app, &player).await;

    for _ in 0..2 {
        let (status, _) = hint(&app, &player).await;
        assert_eq!(status, StatusCode::OK);
    }

    assert_eq!(snapshot(&app, &player).await, before);
}

#[tokio::test]
async fn a_private_table_has_no_hint_route() {
    let (app, _) = app();
    let (owner, _) = private(&app, "Ana", "CONTREE").await;

    let (status, _) = hint(&app, &owner).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_hint_needs_the_players_token() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let uri = format!("/api/tables/{}/hint", player.table);

    let (status, _) = call(&app, get_request(&uri, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let other = solo(&app, "CONTREE").await;
    let (status, _) = hint(
        &app,
        &Player {
            table: player.table.clone(),
            token: other.token,
        },
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn no_hint_while_a_trick_is_under_review() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let mut view = into_card_play(&app, &player).await;
    for _ in 0..8 {
        if view["board"]["reviewingCompletedTrick"] == true {
            break;
        }
        let card = view["board"]["legalCards"][0].clone();
        view = accepted(
            &app,
            &player,
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"], "declare": true}),
        )
        .await;
    }
    assert_eq!(view["board"]["reviewingCompletedTrick"], true);

    let (status, error) = hint(&app, &player).await;

    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(error["error"], "no-hint");
}

#[tokio::test]
async fn no_hint_once_the_round_is_over() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let mut view = into_card_play(&app, &player).await;
    for _ in 0..64 {
        if view["board"]["roundResult"].is_object() {
            break;
        }
        let action = if view["board"]["reviewingCompletedTrick"] == true {
            json!({"type": "continueTrick"})
        } else {
            let card = view["board"]["legalCards"][0].clone();
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"], "declare": true})
        };
        view = accepted(&app, &player, action).await;
    }
    assert!(view["board"]["roundResult"].is_object());

    let (status, error) = hint(&app, &player).await;

    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(error["error"], "no-hint");
}

#[tokio::test]
async fn a_pass_and_play_hint_is_for_the_player_in_turn() {
    let (app, _) = app();
    let player = create(
        &app,
        json!({"kind": "LOCAL", "difficulty": "RELAXED", "variant": "CONTREE",
               "seats": ["Ana", null, "Ben", null]}),
    )
    .await;
    let mut view = snapshot(&app, &player).await;
    let mut asked_for = std::collections::BTreeSet::new();
    for _ in 0..40 {
        if view["board"]["roundResult"].is_object() {
            break;
        }
        if view["board"]["reviewingCompletedTrick"] == true {
            view = accepted(&app, &player, json!({"type": "continueTrick"})).await;
            continue;
        }
        let (status, answer) = hint(&app, &player).await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        asked_for.insert(view["you"].as_u64().unwrap());
        // The answer is what the client would send to make the move.
        let action = match answer["type"].as_str().unwrap() {
            "card" => {
                assert!(
                    view["board"]["legalCards"]
                        .as_array()
                        .unwrap()
                        .contains(&answer["card"])
                );
                json!({"type": "card", "rank": answer["card"]["rank"],
                       "suit": answer["card"]["suit"], "declare": true})
            }
            _ => answer["action"].clone(),
        };
        view = accepted(&app, &player, action).await;
    }
    assert!(
        asked_for.contains(&0) && asked_for.contains(&2),
        "hints for both humans: {asked_for:?}"
    );
}

#[tokio::test]
async fn plays_a_legal_card_from_the_human_hand() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let mut view = accepted(
        &app,
        &player,
        json!({"type": "contract", "value": 160, "suit": "SPADES"}),
    )
    .await;
    while view["bidding"]["complete"] != true {
        view = accepted(&app, &player, json!({"type": "pass"})).await;
    }
    let card = view["board"]["legalCards"][0].clone();
    let played = accepted(
        &app,
        &player,
        json!({"type": "card", "rank": card["rank"], "suit": card["suit"], "declare": true}),
    )
    .await;
    let board = &played["board"];
    assert_eq!(board["hand"].as_array().unwrap().len(), 7);
    assert_eq!(board["currentTrick"][0]["rank"], card["rank"]);
    assert_eq!(board["currentTrick"][0]["suit"], card["suit"]);
}

#[tokio::test]
async fn the_match_score_starts_at_zero_and_a_round_cannot_be_skipped() {
    let (app, _) = app();
    let player = solo(&app, "CONTREE").await;
    let view = snapshot(&app, &player).await;
    assert_eq!(
        view["match"],
        json!({"northSouth": 0, "eastWest": 0, "complete": false, "winner": null, "target": 1000})
    );
    let (status, error) = act(&app, &player, json!({"type": "rematch"})).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rematch before the match ends changes nothing: {error}"
    );
}

#[tokio::test]
async fn answers_not_found_for_an_unknown_table() {
    let (app, _) = app();
    let player = solo(&app, "CLASSIC").await;
    let unknown = uuid::Uuid::new_v4();
    let (status, _) = call(
        &app,
        get_request(&format!("/api/tables/{unknown}"), Some(&player.token)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let uri = format!("/api/tables/{unknown}/actions");
    let (status, _) = call(
        &app,
        json_request("POST", &uri, Some(&player.token), json!({"type": "pass"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        get_request("/api/tables/not-a-uuid", Some(&player.token)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_local_table_needs_its_token() {
    let (app, _) = app();
    let player = solo(&app, "CLASSIC").await;
    let uri = format!("/api/tables/{}", player.table);
    let (status, error) = call(&app, get_request(&uri, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        error,
        json!({"error": "unauthorized", "message": "You are not authorized for this table."})
    );
    let (status, _) = call(&app, get_request(&uri, Some("forged"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn player_token_in_body_is_ignored() {
    let (app, _) = app();
    let (owner, _) = private(&app, "Ana", "CLASSIC").await;
    let uri = format!("/api/tables/{}/actions", owner.table);
    let body = json!({"type": "ready", "ready": true, "playerToken": owner.token});
    let (status, _) = call(&app, json_request("POST", &uri, None, body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &app,
        get_request(&format!("{uri}?playerToken={}", owner.token), None),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_malformed_action_is_a_bad_request() {
    let (app, _) = app();
    let player = solo(&app, "CLASSIC").await;
    let (status, error) = act(&app, &player, json!({"type": "cheat"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"], "bad-request");
    let (status, _) = call(
        &app,
        json_request(
            "POST",
            "/api/tables",
            None,
            json!({"kind": "SECRET", "playerName": "Ana"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// PrivateTableControllerTest

#[tokio::test]
async fn creates_a_waiting_table_with_an_invitation_code_and_owner_seat() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CLASSIC").await;
    assert_eq!(code.len(), 6);
    assert!(
        code.chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='9').contains(&c))
    );
    let view = snapshot(&app, &owner).await;
    assert_eq!(view["you"], 0);
    assert_eq!(view["table"]["invitationCode"], code);
    assert_eq!(view["table"]["status"], "WAITING_FOR_PLAYERS");
    assert_eq!(view["table"]["seats"].as_array().unwrap().len(), 1);
    assert_eq!(view["table"]["seats"][0]["name"], "Ana");
    assert_eq!(view["table"]["seats"][0]["you"], true);
    assert_eq!(view["table"]["publicTable"], false);
    assert!(view["table"]["botFillAt"].is_null());
}

#[tokio::test]
async fn a_private_table_needs_a_player_name() {
    let (app, _) = app();
    let body = json!({"kind": "PRIVATE", "variant": "CLASSIC", "playerName": "  "});
    let (status, error) = call(&app, json_request("POST", "/api/tables", None, body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "Enter a player name.");
}

#[tokio::test]
async fn four_players_can_start_and_coinche_a_private_contree_contract() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CONTREE").await;
    let benoit = join(&app, &code, "Benoit").await;
    let chloe = join(&app, &code, "Chloe").await;
    let david = join(&app, &code, "David").await;
    for player in [&owner, &benoit, &chloe, &david] {
        accepted(&app, player, json!({"type": "ready", "ready": true})).await;
    }
    let started = accepted(
        &app,
        &owner,
        json!({"type": "start", "fillWithBots": false}),
    )
    .await;
    assert_eq!(started["table"]["status"], "IN_PROGRESS");
    assert_eq!(started["bidding"]["hand"].as_array().unwrap().len(), 8);

    let bid = accepted(
        &app,
        &owner,
        json!({"type": "contract", "value": 90, "suit": "HEARTS"}),
    )
    .await;
    assert_eq!(bid["bidding"]["highestBid"], 90);
    assert_eq!(bid["bidding"]["calls"][0]["call"], "90 Hearts");
    assert!(
        bid["bidding"]["auctionId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(bid["bidding"]["auction"][0]["call"], "90 Hearts");

    let coinched = accepted(&app, &benoit, json!({"type": "coinche"})).await;
    assert_eq!(coinched["bidding"]["complete"], true);
    assert_eq!(coinched["bidding"]["calls"][1]["call"], "Coinche!");
    assert_eq!(coinched["you"], 1);

    let board = snapshot(&app, &owner).await["board"].clone();
    assert_eq!(board["contractValue"], 90);
    assert_eq!(board["trump"], "Hearts");
    assert_eq!(board["coinched"], true);
    assert_eq!(board["currentPlayer"], "Ana");
    assert_eq!(board["activePlayerIndex"], 0);
    let card = board["legalCards"][0].clone();
    let played = accepted(
        &app,
        &owner,
        json!({"type": "card", "rank": card["rank"], "suit": card["suit"]}),
    )
    .await;
    assert_eq!(played["board"]["hand"].as_array().unwrap().len(), 7);
    assert_eq!(played["board"]["currentTrick"].as_array().unwrap().len(), 1);
    assert_eq!(played["board"]["activePlayer"], "Benoit");

    let (status, error) = act(
        &app,
        &chloe,
        json!({"type": "card", "rank": "7", "suit": "HEARTS"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "It is not your turn to play.");
}

#[tokio::test]
async fn owner_can_start_with_bots_without_waiting_for_a_full_table() {
    let (app, _) = app();
    let (owner, _) = private(&app, "Ana", "CLASSIC").await;
    accepted(&app, &owner, json!({"type": "ready", "ready": true})).await;
    let started = accepted(&app, &owner, json!({"type": "start", "fillWithBots": true})).await;
    assert_eq!(started["table"]["status"], "IN_PROGRESS");
    assert_eq!(started["table"]["seats"].as_array().unwrap().len(), 4);
    assert_eq!(
        started["table"]["seats"][1]["connectionState"],
        "BOT_TAKEOVER"
    );
    assert_eq!(started["table"]["seats"][1]["ready"], true);
    assert_eq!(started["bidding"]["playerTurn"], true);
}

#[tokio::test]
async fn a_player_sends_a_reaction_that_the_table_lists() {
    let (app, _) = app();
    let (owner, _) = private(&app, "Ana", "CLASSIC").await;
    accepted(&app, &owner, json!({"type": "ready", "ready": true})).await;
    accepted(&app, &owner, json!({"type": "start", "fillWithBots": true})).await;
    let reacted = accepted(
        &app,
        &owner,
        json!({"type": "react", "reaction": "GOOD_GAME"}),
    )
    .await;
    let reactions = reacted["reactions"].as_array().unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0]["position"], 0);
    assert_eq!(reactions[0]["reaction"], "GOOD_GAME");
    assert!(reactions[0]["sequence"].is_number());
}

#[tokio::test]
async fn a_player_moves_to_an_empty_seat() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CLASSIC").await;
    let benoit = join(&app, &code, "Benoit").await;
    let moved = accepted(&app, &benoit, json!({"type": "seat", "position": 2})).await;
    assert_eq!(moved["table"]["seats"][0]["position"], 0);
    assert_eq!(moved["table"]["seats"][1]["name"], "Benoit");
    assert_eq!(moved["table"]["seats"][1]["position"], 2);
    assert_eq!(moved["you"], 2);
    let (status, error) = act(&app, &owner, json!({"type": "seat", "position": 2})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "That seat is taken.");
    assert_eq!(
        snapshot(&app, &benoit).await["you"],
        2,
        "the token follows the player"
    );
}

#[tokio::test]
async fn a_token_from_another_table_is_refused() {
    let (app, _) = app();
    let (table, _) = private(&app, "Ana", "CLASSIC").await;
    let (other, _) = private(&app, "Ben", "CLASSIC").await;
    let uri = format!("/api/tables/{}", table.table);
    let (status, error) = call(&app, get_request(&uri, Some(&other.token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["message"], "You are not authorized for this table.");
}

#[tokio::test]
async fn a_waiting_player_can_leave_and_the_last_one_removes_the_table() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CLASSIC").await;
    let benoit = join(&app, &code, "Benoit").await;
    let left = accepted(&app, &benoit, json!({"type": "leave"})).await;
    assert_eq!(left["table"]["seats"].as_array().unwrap().len(), 1);
    assert_eq!(left["table"]["seats"][0]["name"], "Ana");
    assert!(left["you"].is_null(), "the leaver has no seat any more");

    accepted(&app, &owner, json!({"type": "leave"})).await;
    let (status, _) = call(
        &app,
        get_request(&format!("/api/tables/{}", owner.table), Some(&owner.token)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_owner_sets_a_turn_timer_and_announcements() {
    let (app, _) = app();
    let (owner, code) = private(&app, "Ana", "CLASSIC").await;
    let guest = join(&app, &code, "Benoit").await;
    let set = accepted(&app, &owner, json!({"type": "settings", "turnTimer": 30})).await;
    assert_eq!(set["table"]["turnTimerSeconds"], 30);
    let (status, _) = act(&app, &owner, json!({"type": "settings", "turnTimer": 45})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let off = accepted(
        &app,
        &owner,
        json!({"type": "settings", "announcements": false}),
    )
    .await;
    assert_eq!(off["table"]["announcements"], false);
    let (status, error) = act(
        &app,
        &guest,
        json!({"type": "settings", "announcements": true}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["message"],
        "Only the table owner can change announcements."
    );
}

#[tokio::test]
async fn joining_with_a_wrong_code_is_refused() {
    let (app, _) = app();
    let body = json!({"code": "WRONG2", "playerName": "Ana"});
    let (status, error) = call(&app, json_request("POST", "/api/tables/join", None, body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "The invitation code is invalid.");
}

// MatchmakingControllerTest

#[tokio::test]
async fn quick_match_seats_players_together_at_a_public_table() {
    let (app, _) = app();
    let body = json!({"variant": "CLASSIC", "playerName": "Ana"});
    let (status, first) = call(
        &app,
        json_request("POST", "/api/matchmaking/quick-match", None, body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(first.get("code").is_none_or(Value::is_null));
    let body = json!({"variant": "CLASSIC", "playerName": "Benoit"});
    let (_, second) = call(
        &app,
        json_request("POST", "/api/matchmaking/quick-match", None, body),
    )
    .await;
    assert_eq!(first["tableId"], second["tableId"]);
    let player = Player {
        table: second["tableId"].as_str().unwrap().into(),
        token: second["playerToken"].as_str().unwrap().into(),
    };
    let view = snapshot(&app, &player).await;
    assert_eq!(view["kind"], "PUBLIC");
    assert_eq!(view["table"]["publicTable"], true);
    assert_eq!(view["table"]["botFillAt"], "2026-10-24T12:01:00Z");
    assert_eq!(view["you"], 1);
}

#[tokio::test]
async fn quick_match_rejects_a_blank_name() {
    let (app, _) = app();
    let body = json!({"variant": "CLASSIC", "playerName": " "});
    let (status, error) = call(
        &app,
        json_request("POST", "/api/matchmaking/quick-match", None, body),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["message"], "Enter a player name.");
}

// Match hand-over

#[tokio::test]
async fn a_finished_match_is_handed_to_the_sink() {
    use std::sync::{Arc, Mutex};

    use beelot_server::tables::finished::{FinishedMatch, MatchSink};

    #[derive(Default)]
    struct Recorded(Mutex<Vec<FinishedMatch>>);
    impl MatchSink for Recorded {
        fn record(&self, finished: FinishedMatch) -> futures_util::future::BoxFuture<'_, ()> {
            self.0.lock().unwrap().push(finished);
            Box::pin(async {})
        }
    }

    let recorded = Arc::new(Recorded::default());
    let state = test_state().with_sink(recorded.clone());
    let app = router(state);
    let player = solo(&app, "CONTREE").await;
    let mut view = snapshot(&app, &player).await;
    for guard in 0.. {
        assert!(guard < 2000, "the match did not end");
        if view["match"]["complete"] == true {
            break;
        }
        let action = if view["board"].is_null()
            || view["board"]["roundResult"].is_null() && view["bidding"]["complete"] != true
        {
            if view["bidding"]["highestBid"] == 0 {
                json!({"type": "contract", "value": 80, "suit": "HEARTS"})
            } else {
                json!({"type": "pass"})
            }
        } else if !view["board"]["roundResult"].is_null() {
            json!({"type": "nextRound"})
        } else if view["board"]["reviewingCompletedTrick"] == true {
            json!({"type": "continueTrick"})
        } else {
            let card = &view["board"]["legalCards"][0];
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"]})
        };
        view = accepted(&app, &player, action).await;
    }
    for _ in 0..100 {
        if !recorded.0.lock().unwrap().is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(recorded.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn two_players_and_two_bots_play_a_private_round_to_its_end() {
    let (app, _) = app();
    let (ana, code) = private(&app, "Ana", "CLASSIC").await;
    let ben = join(&app, &code, "Ben").await;
    for player in [&ana, &ben] {
        accepted(&app, player, json!({"type": "ready", "ready": true})).await;
    }
    accepted(&app, &ana, json!({"type": "start", "fillWithBots": true})).await;

    for _ in 0..200 {
        let mut acted = false;
        for player in [&ana, &ben] {
            let view = snapshot(&app, player).await;
            if view["board"]["roundResult"].is_object() {
                return;
            }
            let action = if view["board"]["reviewingCompletedTrick"] == true {
                json!({"type": "continueTrick"})
            } else if view["board"].is_object() {
                let Some(card) = view["board"]["legalCards"].get(0) else {
                    continue;
                };
                json!({"type": "card", "rank": card["rank"], "suit": card["suit"]})
            } else if view["bidding"]["playerTurn"] == true {
                let suit = &view["bidding"]["upturnedCard"]["suit"];
                json!({"type": "trump", "suit": suit})
            } else {
                continue;
            };
            accepted(&app, player, action).await;
            acted = true;
            break;
        }
        if !acted {
            let view = snapshot(&app, &ana).await;
            panic!("neither human can act: {view:#}");
        }
    }
    panic!("the round did not end");
}
