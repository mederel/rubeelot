/// Rule violations reported by the engine.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GameError {
    #[error("it is not this seat's turn to play")]
    NotYourTurn,
    #[error("that card is not a legal play")]
    IllegalCard,
    #[error("the round has ended")]
    RoundEnded,
    #[error("there is no completed trick to continue from")]
    NoCompletedTrick,
}

impl GameError {
    /// A stable kebab-case code, safe to send to clients.
    pub fn code(&self) -> &'static str {
        match self {
            GameError::NotYourTurn => "not-your-turn",
            GameError::IllegalCard => "illegal-card",
            GameError::RoundEnded => "round-ended",
            GameError::NoCompletedTrick => "no-completed-trick",
        }
    }
}
