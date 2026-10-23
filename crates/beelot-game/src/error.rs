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
    #[error("contract bids are only available in Contrée")]
    ContractOnlyInContree,
    #[error("the trump is chosen without a contract value in Classic Belote")]
    TrumpChoiceOnlyInClassic,
    #[error("the auction has ended")]
    AuctionEnded,
    #[error("in the first round only the upturned suit may be accepted")]
    MustTakeUpturnedSuit,
    #[error("the second round needs a suit other than the upturned suit")]
    MustChooseAnotherSuit,
    #[error("a bid must be from 80 to 160 in steps of 10")]
    InvalidBid,
    #[error("a bid must be higher than the current contract")]
    BidTooLow,
    #[error("only an opponent of the declaring team may coinche")]
    CoincheNotAllowed,
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
        }
    }
}
