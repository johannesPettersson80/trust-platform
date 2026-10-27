//! Transfers in parts (specification 34, PORT-05): a request or reply longer than the link's frame
//! travels in parts, on every runtime alike. [`TransferSession`] holds one open upload, in memory or
//! written into a chip's program store (§5.1), and one held reply, [`ReplyParts`] is a reply in the parts of 4,096 bytes that a chip's heap can give,
//! [`IdSequence`] gives their ids, and [`wire`] has the JSON shapes of `transfer.begin`,
//! `transfer.part`, `transfer.status`, `transfer.abort` and `transfer.get`. Nothing here reads a
//! link or a clock: the platform composes these with its control loop.

mod ids;
mod reply_parts;
mod session;
pub mod wire;

#[cfg(test)]
mod reply_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

pub use ids::{IdSequence, TransferId};
#[cfg(test)]
pub(crate) use reply_parts::room_for;
pub use reply_parts::ReplyParts;
pub(crate) use reply_parts::{reserve_part, PartReserve};
pub use session::{
    Complete, Received, ReplyHead, ReplyPart, State, Status, StoredUpload, TransferError,
    TransferSession, HEADER_BYTES, PART_BYTES,
};
