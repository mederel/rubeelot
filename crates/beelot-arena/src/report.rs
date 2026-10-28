//! Results of an arena run, seen from the first bot. Port of the Java `ArenaReport`.

use beelot_game::Variant;

use crate::java_format::fixed;

const Z_95: f64 = 1.96;

/// Results of an arena run, seen from the first bot. Each deal pair is one deal played twice with
/// the teams swapped.
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaReport {
    pub variant: Variant,
    pub announcements: bool,
    /// Whether Contrée deals played all trumps and no trumps (US-064).
    pub all_trumps: bool,
    pub first: String,
    pub second: String,
    pub deal_pairs: usize,
    pub seed: i64,
    /// The first bot's mean point difference per deal.
    pub point_difference: f64,
    /// The half-width of its 95% confidence interval.
    pub point_difference_margin: f64,
    pub first_match_wins: Rate,
    pub first_contracts: Rate,
    pub second_contracts: Rate,
    /// The all trumps and no trumps contracts each bot took, and how many it made.
    pub first_all_trumps_contracts: Rate,
    pub second_all_trumps_contracts: Rate,
    pub first_coinches: Rate,
    pub second_coinches: Rate,
    pub redeals: Rate,
    pub announced_rounds: Rate,
    pub first_timing: Timing,
    pub second_timing: Timing,
}

impl ArenaReport {
    /// The report as the Java arena prints it, ending with a newline.
    pub fn format(&self) -> String {
        let first = &self.first;
        let second = &self.second;
        let difference = self.point_difference;
        let margin = self.point_difference_margin;
        format!(
            "Bot arena — {variant}\n\
             {first} vs {second}, {pairs} duplicate deal pairs, seed {seed}, announcements {announcements}, tout-atout and sans-atout {all_trumps}\n\
             \n\
             Point difference per deal ({first} − {second}): {difference} ± {margin} (95% CI {lower} to {upper})\n\
             Matches to 1,000 won by {first}: {match_wins}\n\
             Contract success: {first} {first_contracts}; {second} {second_contracts}\n\
             Tout-atout and sans-atout contracts: {first} {first_all_trumps}; {second} {second_all_trumps}\n\
             Coinche success: {first} {first_coinches}; {second} {second_coinches}\n\
             Redeal rate: {redeals}\n\
             Rounds with announcements: {announced}\n\
             Decision time per card: {first} {first_timing}; {second} {second_timing}\n",
            variant = self.variant.display_name(),
            pairs = self.deal_pairs,
            seed = self.seed,
            announcements = if self.announcements { "on" } else { "off" },
            all_trumps = if self.all_trumps { "on" } else { "off" },
            difference = fixed(difference, 2, true),
            margin = fixed(margin, 2, false),
            lower = fixed(difference - margin, 2, true),
            upper = fixed(difference + margin, 2, true),
            match_wins = self.first_match_wins.format_with_interval(),
            first_contracts = self.first_contracts.format(),
            second_contracts = self.second_contracts.format(),
            first_all_trumps = self.first_all_trumps_contracts.format(),
            second_all_trumps = self.second_all_trumps_contracts.format(),
            first_coinches = self.first_coinches.format(),
            second_coinches = self.second_coinches.format(),
            redeals = self.redeals.format(),
            announced = self.announced_rounds.format(),
            first_timing = self.first_timing.format(),
            second_timing = self.second_timing.format(),
        )
    }
}

/// Successes out of attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub successes: usize,
    pub attempts: usize,
}

impl Rate {
    pub fn new(successes: usize, attempts: usize) -> Self {
        Self {
            successes,
            attempts,
        }
    }

    /// The success rate, or NaN without attempts.
    pub fn ratio(&self) -> f64 {
        if self.attempts == 0 {
            f64::NAN
        } else {
            self.successes as f64 / self.attempts as f64
        }
    }

    /// Wilson score 95% interval, as `[lower, upper]`.
    pub fn wilson_interval(&self) -> [f64; 2] {
        if self.attempts == 0 {
            return [f64::NAN, f64::NAN];
        }
        let p = self.ratio();
        let z2 = Z_95 * Z_95;
        let attempts = self.attempts as f64;
        let centre = (p + z2 / (2 * self.attempts) as f64) / (1.0 + z2 / attempts);
        let margin = Z_95 * (p * (1.0 - p) / attempts + z2 / (4.0 * attempts * attempts)).sqrt()
            / (1.0 + z2 / attempts);
        [centre - margin, centre + margin]
    }

    /// Java's `%.1f%% (%d/%d)`, or `n/a (0/0)` without attempts.
    pub fn format(&self) -> String {
        if self.attempts == 0 {
            return "n/a (0/0)".to_string();
        }
        format!(
            "{}% ({}/{})",
            fixed(100.0 * self.ratio(), 1, false),
            self.successes,
            self.attempts
        )
    }

    /// The rate followed by its Wilson interval.
    pub fn format_with_interval(&self) -> String {
        if self.attempts == 0 {
            return self.format();
        }
        let [lower, upper] = self.wilson_interval();
        format!(
            "{}, 95% CI {}%–{}%",
            self.format(),
            fixed(100.0 * lower, 1, false),
            fixed(100.0 * upper, 1, false)
        )
    }
}

/// Time a bot takes to choose a card, in microseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub cards: usize,
    pub average_micros: f64,
    pub p99_micros: f64,
}

impl Timing {
    pub fn format(&self) -> String {
        if self.cards == 0 {
            return "n/a".to_string();
        }
        format!(
            "avg {} µs, p99 {} µs",
            fixed(self.average_micros, 1, false),
            fixed(self.p99_micros, 1, false)
        )
    }
}
