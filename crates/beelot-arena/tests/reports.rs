//! Java's number formatting and summation, and the arena reports recorded from the Java arena.

use std::fs;
use std::path::PathBuf;

use beelot_arena::java_format::{fixed, java_sum};
use beelot_arena::{BotArena, Dealing};
use beelot_game::Variant;
use beelot_game::bot::arena::{ARENA_BOTS, arena_bot};

#[test]
fn fixed_rounds_shortest_form_half_up() {
    assert_eq!(fixed(1.005, 2, false), "1.01");
    assert_eq!(fixed(0.125, 2, false), "0.13");
    assert_eq!(fixed(2.5, 0, false), "3");
    assert_eq!(fixed(-0.001, 2, true), "-0.00");
    assert_eq!(fixed(3.0, 2, true), "+3.00");
}

#[test]
fn fixed_handles_carries_zeros_and_large_values() {
    assert_eq!(fixed(9.96, 1, false), "10.0");
    assert_eq!(fixed(0.0, 2, true), "+0.00");
    assert_eq!(fixed(-0.0, 1, false), "-0.0");
    assert_eq!(fixed(0.05, 1, false), "0.1");
    assert_eq!(fixed(123456.0, 1, false), "123456.0");
    assert_eq!(fixed(1e-7, 2, false), "0.00");
    assert_eq!(fixed(-39.3, 2, true), "-39.30");
    assert_eq!(fixed(f64::NAN, 1, false), "NaN");
}

#[test]
fn java_sum_compensates() {
    assert_eq!(java_sum(std::iter::repeat_n(0.1, 10)), 1.0);
}

#[test]
fn java_sum_keeps_an_infinite_simple_sum() {
    assert_eq!(java_sum([f64::INFINITY, 1.0]), f64::INFINITY);
    assert_eq!(java_sum([]), 0.0);
}

/// A fixture's arena run.
struct Configuration {
    first: &'static str,
    second: &'static str,
    variant: Variant,
    announcements: bool,
    all_trumps: bool,
    dealing: Dealing,
}

/// The configuration a fixture's name records:
/// `<first>-vs-<second>-<classic|contree>-<on|off>[-no-all-trumps][-table]`: the announcements,
/// then all trumps and no trumps when they were off, then table dealing. Bot names contain
/// hyphens, so the bots are found among the arena bots.
fn configuration(name: &str) -> Configuration {
    let stem = name.strip_suffix(".txt").expect("a .txt fixture");
    let (stem, dealing) = match stem.strip_suffix("-table") {
        Some(stem) => (stem, Dealing::Table),
        None => (stem, Dealing::Shuffled),
    };
    let (stem, all_trumps) = match stem.strip_suffix("-no-all-trumps") {
        Some(stem) => (stem, false),
        None => (stem, true),
    };
    let (rest, announcements) = if let Some(rest) = stem.strip_suffix("-on") {
        (rest, true)
    } else if let Some(rest) = stem.strip_suffix("-off") {
        (rest, false)
    } else {
        panic!("{name}: announcements are neither on nor off");
    };
    let (bots, variant) = if let Some(bots) = rest.strip_suffix("-classic") {
        (bots, Variant::Classic)
    } else if let Some(bots) = rest.strip_suffix("-contree") {
        (bots, Variant::Contree)
    } else {
        panic!("{name}: unknown variant");
    };
    for first in ARENA_BOTS {
        for second in ARENA_BOTS {
            if format!("{first}-vs-{second}") == bots {
                return Configuration {
                    first,
                    second,
                    variant,
                    announcements,
                    all_trumps,
                    dealing,
                };
            }
        }
    }
    panic!("{name}: unknown bots");
}

fn compared_lines(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter(|line| !line.starts_with("Decision time per card:"))
        .collect()
}

#[test]
fn reports_match_java() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/arena");
    let mut names: Vec<String> = fs::read_dir(&directory)
        .expect("the arena fixtures")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".txt"))
        .collect();
    names.sort();
    assert_eq!(names.len(), 11, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let expected = fs::read_to_string(directory.join(name)).unwrap();
        let run = configuration(name);
        let arena = BotArena::with_dealing(
            arena_bot(run.first).unwrap(),
            arena_bot(run.second).unwrap(),
            run.dealing,
        );
        let actual = arena
            .run(run.variant, 100, 1, run.announcements, run.all_trumps)
            .format();
        if compared_lines(&actual) != compared_lines(&expected) {
            failures.push(format!(
                "{name}:\n--- expected\n{expected}--- actual\n{actual}"
            ));
        }
        assert!(
            actual.ends_with('\n'),
            "{name}: the report ends with a newline"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
