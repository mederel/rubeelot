/// Rule violations reported by the engine. The `Display` text is the kebab-case code, never
/// English: the server maps each code to its message.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    #[error("not-your-turn")]
    NotYourTurn,
    #[error("illegal-card")]
    IllegalCard,
    #[error("round-ended")]
    RoundEnded,
    #[error("no-completed-trick")]
    NoCompletedTrick,
    #[error("contract-only-in-contree")]
    ContractOnlyInContree,
    #[error("trump-choice-only-in-classic")]
    TrumpChoiceOnlyInClassic,
    #[error("auction-ended")]
    AuctionEnded,
    #[error("must-take-upturned-suit")]
    MustTakeUpturnedSuit,
    #[error("must-choose-another-suit")]
    MustChooseAnotherSuit,
    #[error("invalid-bid")]
    InvalidBid,
    #[error("bid-too-low")]
    BidTooLow,
    #[error("coinche-not-allowed")]
    CoincheNotAllowed,
    #[error("all-trumps-not-played")]
    AllTrumpsNotPlayed,
}

impl GameError {
    /// A stable kebab-case code, safe to send to clients.
    pub fn code(&self) -> &'static str {
        match self {
            GameError::NotYourTurn => "not-your-turn",
            GameError::IllegalCard => "illegal-card",
            GameError::RoundEnded => "round-ended",
            GameError::NoCompletedTrick => "no-completed-trick",
            GameError::ContractOnlyInContree => "contract-only-in-contree",
            GameError::TrumpChoiceOnlyInClassic => "trump-choice-only-in-classic",
            GameError::AuctionEnded => "auction-ended",
            GameError::MustTakeUpturnedSuit => "must-take-upturned-suit",
            GameError::MustChooseAnotherSuit => "must-choose-another-suit",
            GameError::InvalidBid => "invalid-bid",
            GameError::BidTooLow => "bid-too-low",
            GameError::CoincheNotAllowed => "coinche-not-allowed",
            GameError::AllTrumpsNotPlayed => "all-trumps-not-played",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GameError::{self, *};

    #[test]
    fn display_is_the_code_for_every_variant() {
        let all: [GameError; 13] = [
            NotYourTurn,
            IllegalCard,
            RoundEnded,
            NoCompletedTrick,
            ContractOnlyInContree,
            TrumpChoiceOnlyInClassic,
            AuctionEnded,
            MustTakeUpturnedSuit,
            MustChooseAnotherSuit,
            InvalidBid,
            BidTooLow,
            CoincheNotAllowed,
            AllTrumpsNotPlayed,
        ];
        for e in all {
            assert_eq!(e.to_string(), e.code());
        }
    }
}
