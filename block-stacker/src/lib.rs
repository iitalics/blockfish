mod ruleset;
mod stacker;

#[allow(unused)]
mod random;

/// One of `"ILJSZTO"`
pub type PieceType = char;
/// One of `"ILJSZTOGH "`
pub type CellColor = char;

pub use ruleset::Ruleset;
pub use stacker::{Config, Stacker};
