//! Times the double-dummy solver on seeded full rounds, as the Java `SolverBenchmark` deals them:
//! `java.util.Random(1)` shuffles the deck, each seat gets eight cards, the generator draws the
//! trump, and the seat after the dealer (deal number modulo 4) leads.

use std::hint::black_box;

use beelot_game::bot::solver::{DoubleDummySolver, Position};
use beelot_game::{Card, JavaRandom, RandomSource, Trump, java_shuffle};
use criterion::{Criterion, criterion_group, criterion_main};

const DEALS: usize = 20;
const SEED: i64 = 1;

fn deals() -> Vec<Position> {
    let mut random = JavaRandom::new(SEED);
    (0..DEALS)
        .map(|deal| {
            let mut deck = Card::deck();
            java_shuffle(&mut deck, &mut random);
            let hands = std::array::from_fn(|seat| deck[seat * 8..seat * 8 + 8].to_vec());
            let trump = Trump::ALL[random.next_int(4) as usize];
            let dealer = deal % 4;
            Position::start(hands, trump, (dealer + 1) % 4)
        })
        .collect()
}

fn full_rounds(c: &mut Criterion) {
    let positions = deals();
    let mut solver = DoubleDummySolver::new();
    let mut group = c.benchmark_group("solver");
    group.sample_size(10);
    group.bench_function("full round, 20 deals", |b| {
        b.iter(|| {
            for position in &positions {
                black_box(solver.solve(black_box(position)));
            }
        })
    });
    group.finish();
    let before = solver.nodes();
    for position in &positions {
        solver.solve(position);
    }
    println!(
        "average positions searched per full round: {}",
        (solver.nodes() - before) / DEALS as u64
    );
}

criterion_group!(benches, full_rounds);
criterion_main!(benches);
