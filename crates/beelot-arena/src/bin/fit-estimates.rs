//! Fits the bots' all trumps and no trumps estimates on seeded bot play (US-064) and prints each
//! feature's weight with its standard error. Port of the Java `EstimateFit` command.

use std::process::ExitCode;

use beelot_arena::fit;
use clap::{Parser, ValueEnum};

/// Fits the all trumps and no trumps estimates on seeded bot play.
#[derive(Parser)]
#[command(version, about)]
struct Options {
    /// The estimate to fit: the taker's or the defence's.
    #[arg(long, value_enum, default_value_t = Stage::Offence)]
    stage: Stage,
    /// The number of seeded deals.
    #[arg(long, default_value_t = 20_000, value_parser = clap::value_parser!(u32).range(1..))]
    deals: u32,
    /// The seed the deals are dealt from.
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    seed: i64,
}

#[derive(Clone, Copy, ValueEnum)]
enum Stage {
    Offence,
    Defence,
}

fn main() -> ExitCode {
    let options = Options::parse();
    let deals = options.deals as usize;
    let report = match options.stage {
        Stage::Offence => fit::offence(deals, options.seed),
        Stage::Defence => fit::defence(deals, options.seed),
    };
    match report {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
