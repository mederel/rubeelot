//! Plays two bots against each other on seeded deals, in the duplicate format: every deal is
//! played twice with the same cards in the same seats, once with the first bot as North–South and
//! once with it as East–West. A deal where everyone passes counts as a redeal worth no points; the
//! cards are not dealt again. Bots always declare their announcements (US-061). Contrée deals
//! play with all trumps and no trumps when asked, as public tables do (US-064); Classic ignores
//! the option. By default every deal pair starts from its own shuffled deck, so that the deals are
//! independent and bot strength is measured on uniformly random hands. With [`Dealing::Table`],
//! each pair is dealt from the cards of the previous pair's first deal, gathered without a shuffle
//! and cut as at a real table (US-085); a redeal or the first pair starts from a shuffled deck.
//! Port of the Java `BotArena`.

use std::time::Instant;

use beelot_game::bidding::BiddingState;
use beelot_game::bot::arena::ArenaBot;
use beelot_game::deck::Deck;
use beelot_game::match_score::MatchScore;
use beelot_game::scoring::RoundResult;
use beelot_game::{Seat, SplitMix64, Team, Variant};

use crate::report::{ArenaReport, Rate, Timing};
use crate::report_format::compensated_sum;

const SEATS: usize = 4;

/// How each deal pair gets its cards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dealing {
    /// From its own shuffled deck.
    #[default]
    Shuffled,
    /// From the previous pair's cards, gathered and cut.
    Table,
}

pub struct BotArena {
    first: Box<dyn ArenaBot>,
    second: Box<dyn ArenaBot>,
    dealing: Dealing,
}

impl BotArena {
    pub fn new(first: Box<dyn ArenaBot>, second: Box<dyn ArenaBot>) -> Self {
        Self::with_dealing(first, second, Dealing::Shuffled)
    }

    pub fn with_dealing(
        first: Box<dyn ArenaBot>,
        second: Box<dyn ArenaBot>,
        dealing: Dealing,
    ) -> Self {
        Self {
            first,
            second,
            dealing,
        }
    }

    /// Plays `deal_pairs` deal pairs, dealt from `seed`, with or without announcements, and with or
    /// without all trumps and no trumps.
    ///
    /// # Panics
    ///
    /// When there are fewer than two deal pairs, as the variance needs two.
    pub fn run(
        &self,
        variant: Variant,
        deal_pairs: usize,
        seed: i64,
        announcements: bool,
        all_trumps: bool,
    ) -> ArenaReport {
        assert!(deal_pairs >= 2, "The arena needs at least two deal pairs.");
        let mut tally = Tally::new(deal_pairs);
        let mut deal_seeds = SplitMix64::new(seed);
        let mut first_north_south_match = MatchScore::new();
        let mut first_east_west_match = MatchScore::new();
        let mut deck: Option<Deck> = None;
        for pair in 0..deal_pairs {
            let deal_seed = deal_seeds.next_long();
            let dealer = pair % SEATS;
            let options = DealOptions {
                variant,
                announcements,
                all_trumps,
                seed: deal_seed,
                dealer,
            };
            let first_north_south = self.play(options, deck.clone(), true, &mut tally);
            let first_east_west = self.play(options, deck.take(), false, &mut tally);
            if self.dealing == Dealing::Table
                && let Deal::Played { gathered, .. } = &first_north_south
            {
                deck = Some(gathered.clone());
            }
            tally.pair(&first_north_south, &first_east_west);
            tally.record_match(&mut first_north_south_match, &first_north_south, true);
            tally.record_match(&mut first_east_west_match, &first_east_west, false);
        }
        let setup = RunSetup {
            variant,
            announcements,
            all_trumps,
            deal_pairs,
            seed,
        };
        tally.report(setup, self.first.name(), self.second.name())
    }

    fn play(
        &self,
        options: DealOptions,
        deck: Option<Deck>,
        first_is_north_south: bool,
        tally: &mut Tally,
    ) -> Deal {
        let mut bidding = BiddingState::dealt_from(
            options.variant,
            options.dealer,
            Box::new(SplitMix64::new(options.seed)),
            deck,
        )
        .with_announcements(options.announcements)
        .with_all_trumps(options.all_trumps);
        let mut bot_random = SplitMix64::new(options.seed);
        while bidding.completed_board().is_none() && bidding.deals() == 1 {
            let seat = bidding.active_seat();
            let (bot, _) = self.seated(seat, first_is_north_south);
            let decision = bot.decide_auction(&bidding.auction_view(seat), &mut bot_random);
            bidding
                .apply(seat, decision)
                .expect("an arena bot makes a legal call");
        }
        let Some(mut board) = bidding.into_board() else {
            return Deal::Redealt;
        };
        let all_trumps_contract = !board.trump().suit_contract();
        let result = loop {
            if let Some(result) = board.round_result() {
                break result.clone();
            }
            if board.reviewing_completed_trick() {
                board
                    .continue_after_trick()
                    .expect("a completed trick is under review");
            } else {
                let seat = board.active_seat();
                let (bot, by_first) = self.seated(seat, first_is_north_south);
                let start = Instant::now();
                let card = bot.choose_card(&board.play_view(seat), &mut bot_random);
                tally.decision(by_first, start.elapsed().as_nanos());
                board
                    .play(seat, card, true)
                    .expect("an arena bot plays a legal card");
            }
        };
        Deal::Played {
            result,
            north_south_declared: Team::of(board.declarer()) == Team::NorthSouth,
            coinched: board.coinched(),
            all_trumps_contract,
            gathered: board
                .gathered_deck((options.dealer + 1) % SEATS)
                .expect("the round is played"),
        }
    }

    /// The bot in the seat, and whether it is the first bot.
    fn seated(&self, seat: Seat, first_is_north_south: bool) -> (&dyn ArenaBot, bool) {
        let by_first = seat.is_multiple_of(2) == first_is_north_south;
        let bot = if by_first { &self.first } else { &self.second };
        (bot.as_ref(), by_first)
    }
}

/// What a run played, for its report.
struct RunSetup {
    variant: Variant,
    announcements: bool,
    all_trumps: bool,
    deal_pairs: usize,
    seed: i64,
}

/// How one deal is dealt and played.
#[derive(Clone, Copy)]
struct DealOptions {
    variant: Variant,
    announcements: bool,
    all_trumps: bool,
    seed: i64,
    dealer: Seat,
}

/// One played deal, or a redeal when everyone passed.
enum Deal {
    Redealt,
    Played {
        result: RoundResult,
        north_south_declared: bool,
        coinched: bool,
        /// Whether the contract is all trumps or no trumps.
        all_trumps_contract: bool,
        /// The cards gathered for the next deal.
        gathered: Deck,
    },
}

impl Deal {
    fn points(&self, north_south: bool) -> i32 {
        match self {
            Deal::Redealt => 0,
            Deal::Played { result, .. } if north_south => result.north_south_awarded,
            Deal::Played { result, .. } => result.east_west_awarded,
        }
    }
}

struct Tally {
    pair_differences: Vec<f64>,
    matches: usize,
    first_match_wins: usize,
    contracts: [usize; 2],
    contracts_made: [usize; 2],
    all_trumps_contracts: [usize; 2],
    all_trumps_contracts_made: [usize; 2],
    coinches: [usize; 2],
    coinches_won: [usize; 2],
    deals: usize,
    redeals: usize,
    announced_rounds: usize,
    first_decisions: Vec<i64>,
    second_decisions: Vec<i64>,
}

impl Tally {
    fn new(deal_pairs: usize) -> Self {
        Self {
            pair_differences: Vec::with_capacity(deal_pairs),
            matches: 0,
            first_match_wins: 0,
            contracts: [0; 2],
            contracts_made: [0; 2],
            all_trumps_contracts: [0; 2],
            all_trumps_contracts_made: [0; 2],
            coinches: [0; 2],
            coinches_won: [0; 2],
            deals: 0,
            redeals: 0,
            announced_rounds: 0,
            first_decisions: Vec::with_capacity(1024),
            second_decisions: Vec::with_capacity(1024),
        }
    }

    fn decision(&mut self, by_first: bool, nanos: u128) {
        let nanos = i64::try_from(nanos).unwrap_or(i64::MAX);
        if by_first {
            self.first_decisions.push(nanos);
        } else {
            self.second_decisions.push(nanos);
        }
    }

    fn pair(&mut self, first_north_south: &Deal, first_east_west: &Deal) {
        let difference = first_north_south.points(true) - first_north_south.points(false)
            + first_east_west.points(false)
            - first_east_west.points(true);
        self.pair_differences.push(f64::from(difference) / 2.0);
        self.contract(first_north_south, true);
        self.contract(first_east_west, false);
    }

    fn contract(&mut self, deal: &Deal, first_is_north_south: bool) {
        self.deals += 1;
        let Deal::Played {
            result,
            north_south_declared,
            coinched,
            all_trumps_contract,
            ..
        } = deal
        else {
            self.redeals += 1;
            return;
        };
        if result.north_south_announcements + result.east_west_announcements > 0 {
            self.announced_rounds += 1;
        }
        let declarer = if *north_south_declared == first_is_north_south {
            0
        } else {
            1
        };
        self.contracts[declarer] += 1;
        if result.contract_made {
            self.contracts_made[declarer] += 1;
        }
        if *all_trumps_contract {
            self.all_trumps_contracts[declarer] += 1;
            if result.contract_made {
                self.all_trumps_contracts_made[declarer] += 1;
            }
        }
        if *coinched {
            self.coinches[1 - declarer] += 1;
            if !result.contract_made {
                self.coinches_won[1 - declarer] += 1;
            }
        }
    }

    /// Records the deal in the running match, and starts a new match once it is won.
    fn record_match(&mut self, score: &mut MatchScore, deal: &Deal, first_is_north_south: bool) {
        let Deal::Played { result, .. } = deal else {
            return;
        };
        score.record(result);
        if !score.complete() {
            return;
        }
        self.matches += 1;
        if (score.winner() == Some(Team::NorthSouth)) == first_is_north_south {
            self.first_match_wins += 1;
        }
        *score = MatchScore::new();
    }

    fn report(self, setup: RunSetup, first: &str, second: &str) -> ArenaReport {
        let RunSetup {
            variant,
            announcements,
            all_trumps,
            deal_pairs,
            seed,
        } = setup;
        let pairs = self.pair_differences.len();
        let mean = compensated_sum(self.pair_differences.iter().copied()) / pairs as f64;
        let variance = compensated_sum(
            self.pair_differences
                .iter()
                .map(|difference| (difference - mean) * (difference - mean)),
        ) / (pairs - 1) as f64;
        let margin = 1.96 * (variance / pairs as f64).sqrt();
        ArenaReport {
            variant,
            announcements,
            all_trumps,
            first: first.to_string(),
            second: second.to_string(),
            deal_pairs,
            seed,
            point_difference: mean,
            point_difference_margin: margin,
            first_match_wins: Rate::new(self.first_match_wins, self.matches),
            first_contracts: Rate::new(self.contracts_made[0], self.contracts[0]),
            second_contracts: Rate::new(self.contracts_made[1], self.contracts[1]),
            first_all_trumps_contracts: Rate::new(
                self.all_trumps_contracts_made[0],
                self.all_trumps_contracts[0],
            ),
            second_all_trumps_contracts: Rate::new(
                self.all_trumps_contracts_made[1],
                self.all_trumps_contracts[1],
            ),
            first_coinches: Rate::new(self.coinches_won[0], self.coinches[0]),
            second_coinches: Rate::new(self.coinches_won[1], self.coinches[1]),
            redeals: Rate::new(self.redeals, self.deals),
            announced_rounds: Rate::new(self.announced_rounds, self.deals - self.redeals),
            first_timing: timing(self.first_decisions),
            second_timing: timing(self.second_decisions),
        }
    }
}

/// The average and 99th percentile of the decision times, in microseconds.
fn timing(mut nanos: Vec<i64>) -> Timing {
    let count = nanos.len();
    if count == 0 {
        return Timing {
            cards: 0,
            average_micros: 0.0,
            p99_micros: 0.0,
        };
    }
    nanos.sort_unstable();
    // `LongStream.average()` divides the integer sum by the count.
    let total: i64 = nanos.iter().fold(0, |sum, &value| sum.wrapping_add(value));
    let average = total as f64 / count as f64 / 1_000.0;
    let p99_index = ((count as f64 * 0.99).ceil() as usize).saturating_sub(1);
    let p99 = nanos[(count - 1).min(p99_index)] as f64 / 1_000.0;
    Timing {
        cards: count,
        average_micros: average,
        p99_micros: p99,
    }
}
