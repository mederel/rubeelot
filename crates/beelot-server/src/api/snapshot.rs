//! What a player sees of a table: one snapshot with the table, the auction, the card play and the match, each part
//! shown from the player's seat. The parts keep the JSON shapes of Java's responses (`PrivateTableResponse`, both
//! `BiddingResponse`s, `GameBoardResponse` and `BoardResponse`, `MatchResponse`, `ReactionResponse`), so the
//! frontend renders them as before. `tests/contract/*.json` holds examples.

use beelot_game::announcements::{self, Announcement};
use beelot_game::bidding::BiddingState;
use beelot_game::board::GameBoard;
use beelot_game::scoring::{Declaration, RoundResult};
use beelot_game::{Card, Difficulty, Suit, Team, Trump, Variant, rules};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::tables::messages::{self, trump_name};
use crate::tables::reactions::TableReaction;
use crate::tables::{Connection, SeatKind, Status, Table, TableKind};

/// Whose view to build: an online player's seat, or a local table's device, which shows the hand of the human in
/// charge of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer {
    Seat(usize),
    Device,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: u64,
    pub table_id: Uuid,
    pub kind: TableKind,
    pub you: Option<usize>,
    pub table: TableView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bidding: Option<BiddingView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardView>,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub r#match: Option<MatchView>,
    pub reactions: Vec<ReactionView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableView {
    pub id: Uuid,
    pub invitation_code: Option<String>,
    pub owner_position: usize,
    pub status: &'static str,
    pub variant: Variant,
    pub variant_label: &'static str,
    pub turn_timer_seconds: u32,
    pub seats: Vec<SeatView>,
    pub public_table: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub bot_fill_at: Option<OffsetDateTime>,
    pub announcements: bool,
    /// Whether Contrée bids may name tout-atout and sans-atout.
    pub all_trumps: bool,
    /// The points the match is played to.
    pub target: i32,
    pub kind: TableKind,
    pub difficulty: Option<Difficulty>,
    pub difficulty_label: Option<&'static str>,
    pub pass_and_play: bool,
    /// Whether the table offers hints (US-065): local tables do.
    pub hints: bool,
}

/// `position` is the seat from 0 to 3 in table order; seats 0 and 2 play North–South.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeatView {
    pub name: String,
    pub ready: bool,
    pub connection_state: &'static str,
    pub position: usize,
    pub you: bool,
    /// `HUMAN` or `BOT`, as Java's bot-game seats had it.
    #[serde(rename = "type")]
    pub r#type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    pub rank: &'static str,
    pub suit: Suit,
    pub symbol: &'static str,
}

impl From<Card> for CardView {
    fn from(card: Card) -> Self {
        Self {
            rank: card.rank.label(),
            suit: card.suit,
            symbol: card.suit.symbol(),
        }
    }
}

fn cards(cards: impl IntoIterator<Item = Card>) -> Vec<CardView> {
    cards.into_iter().map(CardView::from).collect()
}

fn sorted(hand: &[Card], trump: Option<Trump>) -> Vec<CardView> {
    let mut hand = hand.to_vec();
    hand.sort_by_key(|card| card.display_key(trump));
    cards(hand)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCallView {
    pub player_name: String,
    pub call: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionCallView {
    pub player_name: String,
    pub call: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiddingView {
    pub hand: Vec<CardView>,
    pub upturned_card: Option<CardView>,
    pub round: i32,
    pub active_player: String,
    pub player_turn: bool,
    pub message: String,
    pub variant: Variant,
    pub highest_bid: i32,
    /// The suit's name, or empty before the first bid.
    pub highest_bid_suit: &'static str,
    /// The bidder's name, or empty before the first bid.
    pub highest_bidder: String,
    pub coinche_allowed: bool,
    pub complete: bool,
    pub dealer_index: usize,
    /// Each player's latest call of the deal, in table order.
    pub calls: Vec<PlayerCallView>,
    pub current_player_index: usize,
    pub auction_id: Uuid,
    /// Every call of the deal, in the order they were made.
    pub auction: Vec<AuctionCallView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSeatView {
    pub name: String,
    pub card_count: usize,
    pub active: bool,
    pub team: &'static str,
}

/// `suit` is null for a carré.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementView {
    pub kind: announcements::AnnouncementKind,
    pub value: i32,
    pub suit: Option<Suit>,
    pub cards: Vec<CardView>,
    pub top_card: CardView,
}

impl From<&Announcement> for AnnouncementView {
    fn from(announcement: &Announcement) -> Self {
        Self {
            kind: announcement.kind,
            value: announcement.value,
            suit: announcement.suit,
            cards: cards(announcement.cards.iter().copied()),
            top_card: announcement.top_card().into(),
        }
    }
}

/// A player's declared announcements, and whether their team scores them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationView {
    pub seat: usize,
    pub player: String,
    pub team: &'static str,
    pub announcements: Vec<AnnouncementView>,
    pub scored: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResultView {
    pub north_south_card_points: i32,
    pub east_west_card_points: i32,
    pub north_south_dix_de_der: i32,
    pub east_west_dix_de_der: i32,
    pub north_south_belote_bonus: i32,
    pub east_west_belote_bonus: i32,
    pub contract_made: bool,
    pub north_south_awarded: i32,
    pub east_west_awarded: i32,
    pub capot_team: Option<Team>,
    pub north_south_announcements: i32,
    pub east_west_announcements: i32,
    pub announcements: Vec<DeclarationView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardView {
    pub hand: Vec<CardView>,
    pub legal_cards: Vec<CardView>,
    pub seats: Vec<BoardSeatView>,
    pub trump: &'static str,
    pub declaring_team: &'static str,
    pub active_player: String,
    pub completed_tricks: usize,
    pub north_south_score: i32,
    pub east_west_score: i32,
    pub current_trick: Vec<CardView>,
    pub reviewing_completed_trick: bool,
    pub trick_winner: String,
    pub trick_points: i32,
    pub declaration_message: String,
    pub belote_bonus_points: i32,
    pub round_result: Option<RoundResultView>,
    pub variant: Variant,
    pub contract_value: i32,
    pub coinched: bool,
    pub current_player: String,
    pub current_player_index: usize,
    pub active_player_index: usize,
    pub dealer_index: usize,
    pub declaring_player_index: usize,
    pub announcements: bool,
    /// The viewer's announcements, offered with their first card.
    pub own_announcements: Vec<AnnouncementView>,
    /// Who declared announcements, until they are revealed after the first trick.
    pub declarers: Vec<String>,
    pub revealed_announcements: Vec<DeclarationView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchView {
    pub north_south: i32,
    pub east_west: i32,
    pub complete: bool,
    pub winner: Option<Team>,
    pub target: i32,
}

/// A reaction from the seat at `position`; clients show the reactions whose sequence they have not seen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionView {
    pub sequence: u64,
    pub position: usize,
    pub reaction: TableReaction,
}

fn team_name(seat: usize) -> &'static str {
    Team::of(seat).display_name()
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::WaitingForPlayers => "WAITING_FOR_PLAYERS",
        Status::InProgress => "IN_PROGRESS",
    }
}

fn connection_name(connection: Connection) -> &'static str {
    match connection {
        Connection::Connected => "CONNECTED",
        Connection::Disconnected { .. } => "DISCONNECTED",
        Connection::BotTakeover => "BOT_TAKEOVER",
    }
}

/// The table as `viewer` sees it at `now`.
pub fn snapshot(table: &Table, viewer: Viewer, now: OffsetDateTime) -> Snapshot {
    let you = match viewer {
        Viewer::Seat(position) => position,
        Viewer::Device => table.viewer(),
    };
    let name = |seat: usize| {
        table
            .lobby()
            .seat(seat)
            .map(|seat| seat.name.clone())
            .unwrap_or_default()
    };
    Snapshot {
        version: table.version(),
        table_id: table.id,
        kind: table.kind,
        you: table.lobby().seat(you).map(|_| you),
        table: table_view(table, you),
        bidding: table
            .bidding()
            .map(|bidding| bidding_view(table, bidding, you, &name)),
        board: table.board().map(|board| board_view(board, you, &name)),
        r#match: table.match_score().map(|score| MatchView {
            north_south: score.north_south(),
            east_west: score.east_west(),
            complete: score.complete(),
            winner: score.winner(),
            target: score.target(),
        }),
        reactions: table
            .reactions(now)
            .into_iter()
            .map(|reaction| ReactionView {
                sequence: reaction.sequence,
                position: reaction.position,
                reaction: reaction.reaction,
            })
            .collect(),
    }
}

fn table_view(table: &Table, you: usize) -> TableView {
    let lobby = table.lobby();
    let public = lobby.is_public();
    TableView {
        id: table.id,
        invitation_code: table.invitation_code.clone(),
        owner_position: lobby.owner(),
        status: status_name(lobby.status()),
        variant: lobby.variant(),
        variant_label: lobby.variant().display_name(),
        turn_timer_seconds: lobby.turn_timer_seconds(),
        seats: lobby
            .occupied()
            .map(|(position, seat)| SeatView {
                name: seat.name.clone(),
                ready: seat.ready,
                connection_state: connection_name(seat.connection),
                position,
                you: position == you,
                r#type: match seat.kind {
                    SeatKind::Human => "HUMAN",
                    SeatKind::Bot => "BOT",
                },
            })
            .collect(),
        public_table: public,
        bot_fill_at: lobby
            .bot_fill_at()
            .filter(|_| lobby.status() == Status::WaitingForPlayers),
        announcements: lobby.announcements(),
        all_trumps: lobby.all_trumps(),
        target: lobby.target(),
        kind: table.kind,
        difficulty: table.difficulty,
        difficulty_label: table.difficulty.map(difficulty_label),
        pass_and_play: table.pass_and_play(),
        hints: table.kind == TableKind::Local,
    }
}

fn difficulty_label(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Relaxed => "Relaxed",
        Difficulty::Challenging => "Challenging",
    }
}

fn bidding_view(
    table: &Table,
    bidding: &BiddingState,
    you: usize,
    name: &impl Fn(usize) -> String,
) -> BiddingView {
    let active = bidding.active_seat();
    let auction = bidding.auction();
    BiddingView {
        hand: sorted(bidding.hand(you), None),
        upturned_card: bidding.upturned_card().map(CardView::from),
        round: bidding.round(),
        active_player: name(active),
        player_turn: active == you,
        message: messages::auction_message(bidding, table.lobby().variant(), name),
        variant: table.lobby().variant(),
        highest_bid: bidding.highest_bid(),
        highest_bid_suit: bidding.highest_bid_suit().map_or("", trump_name),
        highest_bidder: bidding.highest_bidder().map(name).unwrap_or_default(),
        coinche_allowed: bidding.can_coinche(you),
        complete: bidding.completed_board().is_some(),
        dealer_index: bidding.dealer(),
        calls: (0..4)
            .map(|seat| PlayerCallView {
                player_name: name(seat),
                call: auction
                    .iter()
                    .rev()
                    .find(|call| call.seat == seat)
                    .map(messages::call_label)
                    .unwrap_or_default(),
                active: seat == active,
            })
            .collect(),
        current_player_index: you,
        auction_id: table.auction_id(),
        auction: auction
            .iter()
            .map(|call| AuctionCallView {
                player_name: name(call.seat),
                call: messages::call_label(call),
            })
            .collect(),
    }
}

fn declaration_view(declaration: &Declaration, name: &impl Fn(usize) -> String) -> DeclarationView {
    DeclarationView {
        seat: declaration.seat,
        player: name(declaration.seat),
        team: team_name(declaration.seat),
        announcements: declaration
            .announcements
            .iter()
            .map(AnnouncementView::from)
            .collect(),
        scored: declaration.scored,
    }
}

fn round_result_view(result: &RoundResult, name: &impl Fn(usize) -> String) -> RoundResultView {
    RoundResultView {
        north_south_card_points: result.north_south_card_points,
        east_west_card_points: result.east_west_card_points,
        north_south_dix_de_der: result.north_south_dix_de_der,
        east_west_dix_de_der: result.east_west_dix_de_der,
        north_south_belote_bonus: result.north_south_belote_bonus,
        east_west_belote_bonus: result.east_west_belote_bonus,
        contract_made: result.contract_made,
        north_south_awarded: result.north_south_awarded,
        east_west_awarded: result.east_west_awarded,
        capot_team: result.capot_team,
        north_south_announcements: result.north_south_announcements,
        east_west_announcements: result.east_west_announcements,
        announcements: result
            .announcements
            .iter()
            .map(|declaration| declaration_view(declaration, name))
            .collect(),
    }
}

fn board_view(board: &GameBoard, you: usize, name: &impl Fn(usize) -> String) -> BoardView {
    let trump = board.trump();
    let active = board.active_seat();
    let hand = board.hand(you);
    let reviewing = board.reviewing_completed_trick();
    let visible = board.visible_trick();
    let (north_south_score, east_west_score) = board.scores();
    let announcements = board.play_view(you).announcements;
    BoardView {
        hand: sorted(hand, Some(trump)),
        legal_cards: cards(board.legal_cards(you)),
        seats: (0..4)
            .map(|seat| BoardSeatView {
                name: name(seat),
                card_count: board.hand(seat).len(),
                active: seat == active,
                team: team_name(seat),
            })
            .collect(),
        trump: trump.display_name(),
        declaring_team: team_name(board.declarer()),
        active_player: name(active),
        completed_tricks: board.completed_tricks(),
        north_south_score,
        east_west_score,
        current_trick: cards(visible.iter().map(|played| played.card)),
        reviewing_completed_trick: reviewing,
        trick_winner: if reviewing {
            name(board.next_leader())
        } else {
            String::new()
        },
        trick_points: visible
            .iter()
            .map(|played| rules::points(played.card, trump))
            .sum(),
        declaration_message: messages::declaration_message(board, name),
        belote_bonus_points: board.belote_bonus(),
        round_result: board
            .round_result()
            .map(|result| round_result_view(result, name)),
        variant: board.variant(),
        contract_value: board.contract_value(),
        coinched: board.coinched(),
        current_player: name(you),
        current_player_index: you,
        active_player_index: active,
        dealer_index: board.dealer(),
        declaring_player_index: board.declarer(),
        announcements,
        own_announcements: if announcements && hand.len() == 8 {
            announcements::find(hand, trump)
                .iter()
                .map(AnnouncementView::from)
                .collect()
        } else {
            Vec::new()
        },
        declarers: if board.revealed().is_empty() {
            board
                .declared()
                .iter()
                .map(|entry| name(entry.seat))
                .collect()
        } else {
            Vec::new()
        },
        revealed_announcements: board
            .revealed()
            .iter()
            .map(|declaration| declaration_view(declaration, name))
            .collect(),
    }
}
