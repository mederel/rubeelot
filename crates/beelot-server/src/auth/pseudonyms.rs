//! Names a new account with a random pseudonym, such as "Swift Otter 42", in English, French or Dutch. Pseudonyms
//! are at most 30 characters and not unique: players are identified by their account.

use beelot_game::RandomSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    French,
    Dutch,
}

struct Words {
    adjective_after_noun: bool,
    adjectives: [&'static str; 16],
    animals: [&'static str; 16],
}

impl Words {
    fn name(&self, adjective: &str, animal: &str) -> String {
        if self.adjective_after_noun {
            format!("{animal} {adjective}")
        } else {
            format!("{adjective} {animal}")
        }
    }
}

const ENGLISH: Words = Words {
    adjective_after_noun: false,
    adjectives: [
        "Swift", "Brave", "Clever", "Calm", "Bold", "Lucky", "Witty", "Jolly", "Nimble", "Quiet",
        "Sunny", "Daring", "Gentle", "Mighty", "Plucky", "Wise",
    ],
    animals: [
        "Otter", "Fox", "Badger", "Heron", "Owl", "Lynx", "Panda", "Falcon", "Beaver", "Hedgehog",
        "Dolphin", "Raven", "Tiger", "Koala", "Rabbit", "Squirrel",
    ],
};

/// French adjectives follow the noun; they have the same masculine and feminine form, so they fit every animal.
const FRENCH: Words = Words {
    adjective_after_noun: true,
    adjectives: [
        "Agile",
        "Rapide",
        "Calme",
        "Habile",
        "Brave",
        "Sage",
        "Tenace",
        "Fidèle",
        "Aimable",
        "Espiègle",
        "Intrépide",
        "Héroïque",
        "Magnifique",
        "Superbe",
        "Paisible",
        "Sympathique",
    ],
    animals: [
        "Loutre",
        "Renard",
        "Blaireau",
        "Héron",
        "Hibou",
        "Lynx",
        "Panda",
        "Faucon",
        "Castor",
        "Hérisson",
        "Dauphin",
        "Corbeau",
        "Tigre",
        "Koala",
        "Lapin",
        "Écureuil",
    ],
};

/// Every Dutch animal is a "de" word, so the adjectives take their inflected form.
const DUTCH: Words = Words {
    adjective_after_noun: false,
    adjectives: [
        "Snelle",
        "Dappere",
        "Slimme",
        "Kalme",
        "Stoere",
        "Vrolijke",
        "Wijze",
        "Lenige",
        "Stille",
        "Zonnige",
        "Moedige",
        "Zachte",
        "Machtige",
        "Gelukkige",
        "Sterke",
        "Kwieke",
    ],
    animals: [
        "Otter", "Vos", "Das", "Reiger", "Uil", "Lynx", "Panda", "Valk", "Bever", "Egel",
        "Dolfijn", "Raaf", "Tijger", "Koala", "Haas", "Eekhoorn",
    ],
};

fn words(language: Language) -> &'static Words {
    match language {
        Language::English => &ENGLISH,
        Language::French => &FRENCH,
        Language::Dutch => &DUTCH,
    }
}

/// A new pseudonym in the language, ending with a number from 10 to 99.
pub fn generate(language: Language, rng: &mut dyn RandomSource) -> String {
    let words = words(language);
    let adjective = words.adjectives[rng.next_int(16) as usize];
    let animal = words.animals[rng.next_int(16) as usize];
    format!(
        "{} {}",
        words.name(adjective, animal),
        10 + rng.next_int(90)
    )
}

/// Every pseudonym of a language, without its number.
pub fn all(language: Language) -> Vec<String> {
    let words = words(language);
    words
        .adjectives
        .iter()
        .flat_map(|adjective| {
            words
                .animals
                .iter()
                .map(|animal| words.name(adjective, animal))
        })
        .collect()
}

fn supported(tag: &str) -> Option<Language> {
    match tag.to_ascii_lowercase().as_str() {
        "en" => Some(Language::English),
        "fr" => Some(Language::French),
        "nl" => Some(Language::Dutch),
        _ => None,
    }
}

/// The form's `language` (en, fr, nl) when present, else the best match of `Accept-Language`, else English.
pub fn language(form_language: Option<&str>, accept_language: Option<&str>) -> Language {
    match form_language {
        Some(requested) if !requested.trim().is_empty() => {
            supported(requested).unwrap_or(Language::English)
        }
        _ => accept_language.map_or(Language::English, best_match),
    }
}

/// The best supported language of an `Accept-Language` header, by quality then order, else English.
fn best_match(header: &str) -> Language {
    let mut ranges: Vec<(f32, usize, &str)> = Vec::new();
    for (index, part) in header.split(',').enumerate() {
        let mut pieces = part.split(';');
        let range = pieces.next().unwrap_or("").trim();
        let quality = pieces
            .filter_map(|piece| piece.trim().strip_prefix("q="))
            .next()
            .map_or(Some(1.0), |q| q.trim().parse::<f32>().ok());
        let Some(quality) = quality else {
            return Language::English;
        };
        if range.is_empty() || quality <= 0.0 {
            continue;
        }
        ranges.push((quality, index, range));
    }
    ranges.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    ranges
        .iter()
        .find_map(|(_, _, range)| supported(range.split('-').next().unwrap_or("")))
        .unwrap_or(Language::English)
}

#[cfg(test)]
mod tests {
    //! Tests of the pseudonym generator.

    use beelot_game::SplitMix64;

    use super::*;

    fn without_number(pseudonym: &str) -> String {
        pseudonym.rsplit_once(' ').unwrap().0.to_owned()
    }

    #[test]
    fn names_in_the_requested_language() {
        let french = generate(Language::French, &mut SplitMix64::new(7));
        let parts: Vec<&str> = french.split(' ').collect();
        assert_eq!(parts.len(), 3, "{french}");
        assert!(
            parts[0].chars().next().unwrap().is_uppercase()
                && parts[1].chars().next().unwrap().is_uppercase()
        );
        assert!(
            all(Language::French).contains(&without_number(&french)),
            "{french}"
        );
        let dutch = generate(Language::Dutch, &mut SplitMix64::new(7));
        assert!(
            all(Language::Dutch).contains(&without_number(&dutch)),
            "{dutch}"
        );
    }

    #[test]
    fn every_pseudonym_fits_thirty_characters() {
        for language in [Language::English, Language::French, Language::Dutch] {
            for name in all(language) {
                assert!(format!("{name} 99").chars().count() <= 30, "{name}");
            }
        }
    }

    #[test]
    fn numbers_run_from_ten_to_ninety_nine() {
        let mut rng = SplitMix64::new(1);
        for _ in 0..1000 {
            let name = generate(Language::English, &mut rng);
            let number: u32 = name.rsplit_once(' ').unwrap().1.parse().unwrap();
            assert!((10..=99).contains(&number), "{number}");
        }
    }

    #[test]
    fn picks_the_best_supported_language_of_the_accept_language_header() {
        assert_eq!(language(None, Some("de-DE, nl;q=0.9")), Language::Dutch);
        assert_eq!(
            language(None, Some("fr-BE,fr;q=0.9,en;q=0.8")),
            Language::French
        );
        assert_eq!(language(None, Some("de-DE")), Language::English);
        assert_eq!(language(None, Some("en;q=0.5, fr;q=0.8")), Language::French);
    }

    #[test]
    fn uses_english_without_a_usable_header() {
        assert_eq!(language(None, None), Language::English);
        assert_eq!(language(None, Some("")), Language::English);
        assert_eq!(language(None, Some(";;q=nonsense")), Language::English);
    }

    #[test]
    fn the_language_comes_from_the_form_then_from_accept_language() {
        assert_eq!(language(Some("fr"), Some("nl")), Language::French);
        assert_eq!(language(None, Some("nl-BE")), Language::Dutch);
        assert_eq!(language(Some("de"), Some("nl")), Language::English);
        assert_eq!(language(Some(" "), Some("nl")), Language::Dutch);
    }
}
