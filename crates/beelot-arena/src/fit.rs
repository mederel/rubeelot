//! Fits the bots' all trumps and no trumps estimates on bot play (US-064). The offence stage plays
//! each seeded deal twice, as an all trumps and as a no trumps contract at 80 taken by North, and
//! fits the points North–South make (cards, dix de der and Belote) to the features of North's
//! hand. The defence stage plays seeded Contrée auctions with the option on, the bots' coinches
//! read as passes so that the coinche being fitted doesn't select the deals; on each all trumps or
//! no trumps contract, each defender's hand is one sample, and 162 less the taker's total (cards,
//! dix de der, Belote and announcements) is fitted to its features and the contract value: the
//! contract fails when that exceeds 162 less the contract, which is what the coinche test asks.
//! The weights are copied into the rule-based bot.

use std::fmt::Write;

use beelot_game::bidding::{AuctionDecision, BiddingState};
use beelot_game::board::GameBoard;
use beelot_game::bot::BotStrategy;
use beelot_game::bot::hand_features::{self, Feature};
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::bot::turns::play_turn;
use beelot_game::rng::shuffle;
use beelot_game::scoring::RoundResult;
use beelot_game::{Card, SplitMix64, Trump, Variant};

use crate::report_format::fixed;

const PIVOT_TOLERANCE: f64 = 1e-9;

/// The fitted intercept and weights, each weight's standard error, and the share of the variance
/// explained.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub intercept: f64,
    pub weights: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub r_squared: f64,
}

/// Ordinary least squares with an intercept: fits the targets to the rows' features. The rows
/// don't hold the intercept column.
pub fn least_squares(rows: &[Vec<f64>], targets: &[f64]) -> Result<Fit, &'static str> {
    let n = rows.len();
    let p = rows.first().map_or(0, Vec::len) + 1;
    if n <= p {
        return Err("A fit needs more samples than weights.");
    }
    let mut normal = vec![vec![0.0; p]; p];
    let mut moments = vec![0.0; p];
    for (row, &target) in rows.iter().zip(targets) {
        let x = with_intercept(row);
        for i in 0..p {
            moments[i] += x[i] * target;
            for j in 0..p {
                normal[i][j] += x[i] * x[j];
            }
        }
    }
    let inverse = invert(&normal)?;
    let mut beta = vec![0.0; p];
    for i in 0..p {
        for j in 0..p {
            beta[i] += inverse[i][j] * moments[j];
        }
    }
    let mut mean = 0.0;
    for &target in targets {
        mean += target / n as f64;
    }
    let mut residuals = 0.0;
    let mut total = 0.0;
    for (row, &target) in rows.iter().zip(targets) {
        let x = with_intercept(row);
        let mut predicted = 0.0;
        for i in 0..p {
            predicted += beta[i] * x[i];
        }
        residuals += (target - predicted) * (target - predicted);
        total += (target - mean) * (target - mean);
    }
    let variance = residuals / (n - p) as f64;
    Ok(Fit {
        intercept: beta[0],
        weights: beta[1..].to_vec(),
        standard_errors: (1..p).map(|i| (variance * inverse[i][i]).sqrt()).collect(),
        r_squared: if total == 0.0 {
            0.0
        } else {
            1.0 - residuals / total
        },
    })
}

fn with_intercept(features: &[f64]) -> Vec<f64> {
    let mut x = Vec::with_capacity(features.len() + 1);
    x.push(1.0);
    x.extend_from_slice(features);
    x
}

/// Gauss–Jordan elimination with partial pivoting.
fn invert(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, &'static str> {
    let size = matrix.len();
    let mut a: Vec<Vec<f64>> = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut augmented = row.clone();
            augmented.resize(2 * size, 0.0);
            augmented[size + i] = 1.0;
            augmented
        })
        .collect();
    for column in 0..size {
        let mut pivot = column;
        for row in column + 1..size {
            if a[row][column].abs() > a[pivot][column].abs() {
                pivot = row;
            }
        }
        if a[pivot][column].abs() < PIVOT_TOLERANCE {
            return Err("Singular fit");
        }
        a.swap(column, pivot);
        let scale = a[column][column];
        for value in &mut a[column] {
            *value /= scale;
        }
        for row in 0..size {
            if row == column || a[row][column] == 0.0 {
                continue;
            }
            let factor = a[row][column];
            let pivot_row = a[column].clone();
            for (value, pivot) in a[row].iter_mut().zip(&pivot_row) {
                *value -= factor * pivot;
            }
        }
    }
    Ok(a.into_iter().map(|row| row[size..].to_vec()).collect())
}

/// The offence fits, as the report prints them.
pub fn offence(deals: usize, seed: i64) -> Result<String, &'static str> {
    let strategy = RuleBasedStrategy::new();
    let mut report = String::new();
    for trump in [Trump::AllTrumps, Trump::NoTrumps] {
        let features = hand_features::offence(trump);
        let mut deal_seeds = SplitMix64::new(seed);
        let mut rows = Vec::with_capacity(deals);
        let mut targets = Vec::with_capacity(deals);
        for _ in 0..deals {
            let hands = deal(deal_seeds.next_long());
            let north = counts(features, &hands[0]);
            let result = play_out(
                GameBoard::from_contract(hands, trump, 0, 80, false, 3, true),
                &strategy,
            );
            rows.push(north);
            targets.push(f64::from(
                result.north_south_card_points
                    + result.north_south_dix_de_der
                    + result.north_south_belote_bonus,
            ));
        }
        print(
            &mut report,
            &format!(
                "{} offence, {deals} deals, seed {seed}",
                trump.display_name()
            ),
            features,
            &least_squares(&rows, &targets)?,
            &[],
        );
    }
    Ok(report)
}

/// The defence fits, as the report prints them.
pub fn defence(deals: usize, seed: i64) -> Result<String, &'static str> {
    let strategy = RuleBasedStrategy::new();
    // All trumps, then no trumps.
    let mut rows: [Vec<Vec<f64>>; 2] = Default::default();
    let mut targets: [Vec<f64>; 2] = Default::default();
    let mut deal_seeds = SplitMix64::new(seed);
    for deal in 0..deals {
        let mut bidding = BiddingState::new(
            Variant::Contree,
            deal % 4,
            Box::new(SplitMix64::new(deal_seeds.next_long())),
        )
        .with_announcements(true)
        .with_all_trumps(true);
        while bidding.completed_board().is_none() && bidding.deals() == 1 {
            let seat = bidding.active_seat();
            let decision = strategy.decide_auction(&bidding.auction_view(seat));
            let applied = if decision == AuctionDecision::Coinche {
                bidding.pass(seat)
            } else {
                bidding.apply(seat, decision)
            };
            applied.expect("the bot makes a legal call");
        }
        let Some(board) = bidding.into_board() else {
            continue;
        };
        let trump = board.trump();
        let index = match trump {
            Trump::AllTrumps => 0,
            Trump::NoTrumps => 1,
            _ => continue,
        };
        let features = hand_features::defence(trump);
        let defence = (board.declarer() + 1) % 2;
        let samples: Vec<Vec<f64>> = (defence..4)
            .step_by(2)
            .map(|seat| {
                let mut row = counts(features, &board.play_view(seat).hand);
                row.push(f64::from(board.contract_value()));
                row
            })
            .collect();
        let result = play_out(board, &strategy);
        let taker_total = if defence == 1 {
            result.north_south_card_points
                + result.north_south_dix_de_der
                + result.north_south_belote_bonus
                + result.north_south_announcements
        } else {
            result.east_west_card_points
                + result.east_west_dix_de_der
                + result.east_west_belote_bonus
                + result.east_west_announcements
        };
        let points = f64::from(162 - taker_total);
        for row in samples {
            rows[index].push(row);
            targets[index].push(points);
        }
    }
    let mut report = String::new();
    for (index, trump) in [Trump::AllTrumps, Trump::NoTrumps].into_iter().enumerate() {
        let samples = &rows[index];
        if samples.is_empty() {
            continue;
        }
        print(
            &mut report,
            &format!(
                "{} defence, {deals} deals, seed {seed}, {} defender hands",
                trump.display_name(),
                samples.len()
            ),
            hand_features::defence(trump),
            &least_squares(samples, &targets[index])?,
            &["contract value"],
        );
    }
    Ok(report)
}

/// Four hands of eight cards from a deck shuffled from the seed.
fn deal(deal_seed: i64) -> [Vec<Card>; 4] {
    let mut deck = Card::deck();
    shuffle(&mut deck, &mut SplitMix64::new(deal_seed));
    std::array::from_fn(|seat| deck[8 * seat..8 * seat + 8].to_vec())
}

fn play_out(mut board: GameBoard, strategy: &dyn BotStrategy) -> RoundResult {
    loop {
        if let Some(result) = board.round_result() {
            return result.clone();
        }
        if board.reviewing_completed_trick() {
            board
                .continue_after_trick()
                .expect("a completed trick is under review");
        } else {
            play_turn(&mut board, strategy).expect("the bot plays a legal card");
        }
    }
}

fn counts(features: &[Feature], hand: &[Card]) -> Vec<f64> {
    features
        .iter()
        .map(|feature| f64::from((feature.count)(hand)))
        .collect()
}

/// One line per feature, then the R².
fn print(report: &mut String, title: &str, features: &[Feature], fit: &Fit, extra_names: &[&str]) {
    let names = features
        .iter()
        .map(|feature| feature.name)
        .chain(extra_names.iter().copied());
    writeln!(report, "{title}").unwrap();
    writeln!(
        report,
        "  {:<28} {:>8}",
        "intercept",
        fixed(fit.intercept, 2, false)
    )
    .unwrap();
    for (index, name) in names.enumerate() {
        writeln!(
            report,
            "  {name:<28} {:>8} ± {}",
            fixed(fit.weights[index], 2, false),
            fixed(fit.standard_errors[index], 2, false)
        )
        .unwrap();
    }
    writeln!(report, "  R² {}\n", fixed(fit.r_squared, 3, false)).unwrap();
}

#[cfg(test)]
mod tests {
    //! Tests of the least-squares fit.

    use super::*;
    use beelot_game::rng::RandomSource;

    #[test]
    fn recovers_known_weights() {
        let mut random = SplitMix64::new(1);
        let mut rows = Vec::new();
        let mut targets = Vec::new();
        for index in 0..500 {
            let first = f64::from(random.next_int(5));
            let second = f64::from(random.next_int(5));
            rows.push(vec![first, second]);
            // A small deterministic noise between -1 and 1.
            let noise = f64::from(index % 21 - 10) / 10.0;
            targets.push(50.0 + 7.0 * first - 3.0 * second + noise);
        }

        let fit = least_squares(&rows, &targets).unwrap();

        assert!((fit.intercept - 50.0).abs() < 0.5, "{fit:?}");
        assert!((fit.weights[0] - 7.0).abs() < 0.1, "{fit:?}");
        assert!((fit.weights[1] + 3.0).abs() < 0.1, "{fit:?}");
        assert!(
            fit.standard_errors[0] > 0.0 && fit.standard_errors[0] < 0.1,
            "{fit:?}"
        );
        assert!(fit.r_squared > 0.95, "{fit:?}");
    }

    #[test]
    fn refuses_a_feature_that_never_varies() {
        let rows = [[1.0, 0.0], [2.0, 0.0], [3.0, 0.0], [4.0, 0.0]].map(Vec::from);

        assert_eq!(
            least_squares(&rows, &[1.0, 2.0, 3.0, 4.0]),
            Err("Singular fit")
        );
    }

    #[test]
    fn refuses_too_few_samples() {
        assert_eq!(
            least_squares(&[vec![1.0], vec![2.0]], &[1.0, 2.0]),
            Err("A fit needs more samples than weights.")
        );
    }
}
