use beelot_game::{Card, JavaRandom, RandomSource, Rank, SplitMix64, Suit, java_shuffle};
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/rng.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn draws(entry: &Value) -> Vec<(i32, i32)> {
    entry["draws"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                pair[0].as_i64().unwrap() as i32,
                pair[1].as_i64().unwrap() as i32,
            )
        })
        .collect()
}

#[test]
fn java_random_matches_java() {
    for entry in fixture()["javaRandom"].as_array().unwrap() {
        let mut rng = JavaRandom::new(entry["seed"].as_i64().unwrap());
        for (bound, expected) in draws(entry) {
            assert_eq!(rng.next_int(bound), expected, "entry {entry}");
        }
    }
}

#[test]
fn java_shuffle_matches_java() {
    for entry in fixture()["javaShuffle"].as_array().unwrap() {
        let mut deck = Card::deck();
        java_shuffle(
            &mut deck,
            &mut JavaRandom::new(entry["seed"].as_i64().unwrap()),
        );
        let expected: Vec<Card> = entry["deck"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().parse().unwrap())
            .collect();
        assert_eq!(deck, expected);
    }
}

#[test]
fn split_mix_matches_java() {
    for entry in fixture()["splitMix"].as_array().unwrap() {
        let mut rng = SplitMix64::new(entry["seed"].as_i64().unwrap());
        for expected in entry["longs"].as_array().unwrap() {
            assert_eq!(rng.next_long(), expected.as_i64().unwrap());
        }
        for (bound, expected) in draws(entry) {
            assert_eq!(rng.next_int(bound), expected);
        }
    }
}

#[test]
fn cards_round_trip() {
    let card: Card = "10H".parse().unwrap();
    assert_eq!(card, Card::new(Rank::Ten, Suit::Hearts));
    assert_eq!(card.to_string(), "10H");
    assert!("1H".parse::<Card>().is_err());
}
