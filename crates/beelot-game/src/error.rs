/// Rule violations reported by the engine. Variants arrive with the rules and bidding.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GameError {}

impl GameError {
    pub fn code(&self) -> &'static str {
        match *self {}
    }
}
