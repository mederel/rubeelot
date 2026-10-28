//! A short arena run, checking that the arena still works. Port of the Java `BotArenaTest`;
//! longer runs use the `beelot-arena` command.

use beelot_arena::{ArenaReport, BotArena, Rate};
use beelot_game::Variant;
use beelot_game::bot::arena::arena_bot;

const DEAL_PAIRS: usize = 200;

fn arena(first: &str, second: &str) -> BotArena {
    BotArena::new(arena_bot(first).unwrap(), arena_bot(second).unwrap())
}

fn run(first: &str, second: &str, variant: Variant, seed: i64) -> ArenaReport {
    arena(first, second).run(variant, DEAL_PAIRS, seed, true, true)
}

#[test]
fn a_bot_playing_itself_scores_even_on_every_deal_pair() {
    let report = run("random", "random", Variant::Contree, 7);

    assert_eq!(report.point_difference, 0.0);
    assert_eq!(report.point_difference_margin, 0.0);
}

#[test]
fn the_same_seed_gives_the_same_results() {
    let arena = arena("current", "random");

    let first_run = arena.run(Variant::Contree, DEAL_PAIRS, 42, true, true);
    let second_run = arena.run(Variant::Contree, DEAL_PAIRS, 42, true, true);

    assert_eq!(first_run.point_difference, second_run.point_difference);
    assert_eq!(first_run.first_match_wins, second_run.first_match_wins);
    assert_eq!(first_run.first_contracts, second_run.first_contracts);
}

#[test]
fn the_current_bot_beats_random_card_play_in_contree() {
    let report = run("current", "random", Variant::Contree, 1);

    assert!(report.point_difference > 0.0, "{}", report.format());
    // Bots only open with a strong enough hand, so some deals are redealt.
    let played = 2 * DEAL_PAIRS - report.redeals.successes;
    assert_eq!(
        played,
        report.first_contracts.attempts + report.second_contracts.attempts
    );
    assert!(report.first_match_wins.attempts > 0, "{}", report.format());
    // The last trick is played automatically, so each bot chooses seven cards for each of its two seats.
    assert_eq!(played * 14, report.first_timing.cards);
}

#[test]
fn bots_take_and_play_classic_contracts() {
    let report = run("current", "random", Variant::Classic, 1);

    assert!(
        report.redeals.successes < report.redeals.attempts,
        "some classic deals are taken"
    );
    assert!(report.first_contracts.attempts > 0);
    assert!(
        report.first_timing.cards > 0,
        "classic contracts are played out"
    );
}

#[test]
fn current_bidding_beats_the_baseline_bidding() {
    let report = run("current", "baseline", Variant::Classic, 1);

    assert!(
        report.point_difference - report.point_difference_margin > 0.0,
        "{}",
        report.format()
    );
}

#[test]
fn current_card_play_beats_the_previous_card_play() {
    let report = run("current", "previous-play", Variant::Contree, 1);

    assert!(
        report.point_difference - report.point_difference_margin > 0.0,
        "{}",
        report.format()
    );
}

#[test]
fn the_report_says_whether_announcements_were_on() {
    let arena = arena("current", "current");

    let on = arena.run(Variant::Contree, 2, 1, true, true);
    let off = arena.run(Variant::Contree, 2, 1, false, true);

    assert!(on.announcements);
    assert!(on.format().contains("announcements on"), "{}", on.format());
    assert!(!off.announcements);
    assert!(
        off.format().contains("announcements off"),
        "{}",
        off.format()
    );
}

#[test]
fn bots_declare_their_announcements_in_the_arena() {
    let arena = arena("current", "current");

    let on = arena.run(Variant::Contree, DEAL_PAIRS, 1, true, true);
    let off = arena.run(Variant::Contree, DEAL_PAIRS, 1, false, true);

    assert!(on.announced_rounds.successes > 0, "{}", on.format());
    assert!(
        on.format().contains("Rounds with announcements: "),
        "{}",
        on.format()
    );
    assert_eq!(off.announced_rounds.successes, 0);
}

#[test]
fn classic_ignores_the_all_trumps_option() {
    let arena = arena("current", "random");

    let on = arena.run(Variant::Classic, DEAL_PAIRS, 1, true, true);
    let off = arena.run(Variant::Classic, DEAL_PAIRS, 1, true, false);

    assert_eq!(off.point_difference, on.point_difference);
    assert_eq!(off.first_contracts, on.first_contracts);
}

#[test]
fn the_report_says_whether_all_trumps_were_on() {
    let arena = arena("current", "current");

    let on = arena.run(Variant::Contree, 2, 1, true, true);
    let off = arena.run(Variant::Contree, 2, 1, true, false);

    assert!(on.all_trumps);
    assert!(
        on.format().contains("tout-atout and sans-atout on"),
        "{}",
        on.format()
    );
    assert!(
        on.format()
            .contains("Tout-atout and sans-atout contracts: "),
        "{}",
        on.format()
    );
    assert!(!off.all_trumps);
    assert!(
        off.format().contains("tout-atout and sans-atout off"),
        "{}",
        off.format()
    );
}

#[test]
fn with_the_option_off_no_bot_declares_all_trumps() {
    let off =
        arena("current", "no-all-trumps-bidding").run(Variant::Contree, DEAL_PAIRS, 1, true, false);

    assert_eq!(off.first_all_trumps_contracts.attempts, 0);
    assert_eq!(off.second_all_trumps_contracts.attempts, 0);
}

#[test]
fn bots_bid_all_trumps_and_no_trumps_in_the_arena() {
    let report = run("current", "no-all-trumps-bidding", Variant::Contree, 1);

    assert!(
        report.first_all_trumps_contracts.attempts > 0,
        "{}",
        report.format()
    );
    assert_eq!(
        report.second_all_trumps_contracts.attempts,
        0,
        "{}",
        report.format()
    );
}

#[test]
#[should_panic(expected = "at least two deal pairs")]
fn the_arena_needs_two_deal_pairs() {
    arena("current", "random").run(Variant::Contree, 1, 1, true, true);
}

#[test]
fn the_no_announcement_bidding_bot_is_available() {
    assert_eq!(
        arena_bot("no-announcement-bidding").unwrap().name(),
        "no-announcement-bidding"
    );
}

#[test]
fn wilson_interval_surrounds_the_observed_rate() {
    let [lower, upper] = Rate::new(60, 100).wilson_interval();

    assert!((lower - 0.502).abs() < 0.001, "{lower}");
    assert!((upper - 0.691).abs() < 0.001, "{upper}");
}
