//! Runs the bot arena from the command line, in both variants by default, and writes the report to
//! a file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use beelot_arena::{BotArena, Dealing};
use beelot_game::Variant;
use beelot_game::bot::arena::arena_bot;
use clap::{Parser, ValueEnum};

/// Plays two arena bots against each other on seeded duplicate deals.
#[derive(Parser)]
#[command(version, about)]
struct Options {
    /// The first bot, whose results the report shows.
    #[arg(long, default_value = "current")]
    first: String,
    /// The second bot.
    #[arg(long, default_value = "random")]
    second: String,
    /// The number of duplicate deal pairs, at least two.
    #[arg(long, default_value_t = 10_000, value_parser = clap::value_parser!(u32).range(2..))]
    deals: u32,
    /// The seed the deals are dealt from.
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    seed: i64,
    /// The variant, or both one after the other.
    #[arg(long, value_enum, default_value_t = VariantChoice::Both)]
    variant: VariantChoice,
    /// Whether players declare announcements.
    #[arg(long, value_enum, default_value_t = Switch::On)]
    announcements: Switch,
    /// Whether Contrée deals play all trumps and no trumps, as public tables do.
    #[arg(long, value_enum, default_value_t = Switch::On)]
    all_trumps: Switch,
    /// How each deal pair gets its cards: its own shuffled deck, so that the deals are
    /// independent, or the previous pair's cards, gathered and cut as at a real table.
    #[arg(long, value_enum, default_value_t = DealingChoice::Shuffled)]
    dealing: DealingChoice,
    /// Where to write the report.
    #[arg(long, default_value = "target/bot-arena/report.txt")]
    report: PathBuf,
}

#[derive(Clone, Copy, ValueEnum)]
enum VariantChoice {
    Classic,
    Contree,
    Both,
}

impl VariantChoice {
    fn variants(self) -> &'static [Variant] {
        match self {
            VariantChoice::Classic => &[Variant::Classic],
            VariantChoice::Contree => &[Variant::Contree],
            VariantChoice::Both => &[Variant::Classic, Variant::Contree],
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum DealingChoice {
    Shuffled,
    Table,
}

#[derive(Clone, Copy, ValueEnum)]
enum Switch {
    On,
    Off,
}

fn main() -> ExitCode {
    match run(Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<(), String> {
    let first = arena_bot(&options.first).map_err(|error| error.to_string())?;
    let second = arena_bot(&options.second).map_err(|error| error.to_string())?;
    let dealing = match options.dealing {
        DealingChoice::Shuffled => Dealing::Shuffled,
        DealingChoice::Table => Dealing::Table,
    };
    let arena = BotArena::with_dealing(first, second, dealing);
    let announcements = matches!(options.announcements, Switch::On);
    let all_trumps = matches!(options.all_trumps, Switch::On);
    let mut report = String::new();
    for &variant in options.variant.variants() {
        let section = arena
            .run(
                variant,
                options.deals as usize,
                options.seed,
                announcements,
                all_trumps,
            )
            .format();
        let section = if dealing == Dealing::Table {
            format!("Dealing: from the gathered tricks, cut, no shuffle\n{section}")
        } else {
            section
        };
        println!("{section}");
        report.push_str(&section);
        report.push('\n');
    }
    write_report(&options.report, &report)
}

fn write_report(path: &Path, report: &str) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("cannot write {}: {error}", path.display());
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(failed)?;
    }
    fs::write(path, report).map_err(failed)?;
    let absolute = std::path::absolute(path).map_err(failed)?;
    println!("Report written to {}", absolute.display());
    Ok(())
}
