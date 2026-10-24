//! Invitation codes for private tables.

use beelot_game::RandomSource;

use super::OsRandom;

/// Letters and digits that cannot be mistaken for one another: no I, O, 0 or 1.
pub const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// A new 6-character code that `taken` doesn't know.
pub fn new_code(mut taken: impl FnMut(&str) -> bool) -> String {
    let alphabet = ALPHABET.as_bytes();
    loop {
        let code: String = (0..6)
            .map(|_| alphabet[OsRandom.next_int(alphabet.len() as i32) as usize] as char)
            .collect();
        if !taken(&code) {
            return code;
        }
    }
}

/// How a code typed by a player is looked up: trimmed and upper-cased. Anything over 32 characters matches nothing.
pub fn normalized(code: &str) -> Option<String> {
    (code.len() <= 32).then(|| code.trim().to_uppercase())
}
