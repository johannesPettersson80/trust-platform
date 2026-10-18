//! Transfers in parts (specification 34, PORT-05): a request or reply longer than the link's frame
//! travels in parts, on every runtime alike. [`TransferSession`] holds one open upload and one held
//! reply, [`IdSequence`] gives their ids, and [`wire`] has the JSON shapes of `transfer.begin`,
//! `transfer.part`, `transfer.status`, `transfer.abort` and `transfer.get`. Nothing here reads a
//! link or a clock: the platform composes these with its control loop.

mod ids;
mod session;
pub mod wire;

#[cfg(test)]
mod tests;

pub use ids::{IdSequence, TransferId};
pub use session::{
    ReplyHead, ReplyPart, State, Status, TransferError, TransferSession, PART_BYTES,
};
