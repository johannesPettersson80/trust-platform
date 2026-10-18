//! The transfers of one boot (specification 34, PORT-05): one open upload, staged in RAM in parts
//! until its last byte arrives, and one held reply, kept whole and read back in parts. An upload
//! reserves both of its buffers at begin, so a part allocates nothing. The rules here hold on every
//! runtime; their JSON shapes are in [`crate::transfer::wire`].

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::{DecodeSliceError, Engine as _};
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;

use super::ids::{IdSequence, TransferId};

/// The length of every part of an upload but the last, and the longest part a held reply is read
/// in (specification 34, PORT-05).
pub const PART_BYTES: usize = 4_096;

/// Why a transfer operation was refused. A refusal changes nothing, except that an upload whose
/// SHA-256 does not match is dropped (see [`TransferError::Sha256Mismatch`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferError {
    /// Another upload is open: `transfer busy: <id>`.
    Busy(TransferId),
    /// The size is above the device's maximum: `transfer too large: <bytes>, the device takes at
    /// most <max>`. Nothing was reserved.
    TooLarge {
        /// The size asked for.
        bytes: u64,
        /// The device's maximum.
        max: usize,
    },
    /// No open upload or held reply has this id: `unknown transfer`.
    Unknown,
    /// The reply is longer than the reply buffer holds: `reply longer than <capacity> bytes`. It
    /// is refused whole, never cut.
    ReplyTooLong {
        /// The reply buffer's capacity.
        capacity: usize,
    },
    /// This boot has no sequence number left: `transfer ids are used up until the next boot`.
    IdsUsedUp,
    /// The heap cannot hold the upload's buffers: `no room for a transfer of <bytes> bytes`. The
    /// refusal is an error, never an abort, and nothing stays reserved.
    NoRoom {
        /// The size of the upload asked for.
        bytes: u64,
    },
    /// The part's data is not canonical standard base64: `part at offset <offset> is not canonical
    /// base64`. Nothing is stored.
    PartData {
        /// The offset the part names.
        offset: u64,
    },
    /// The part's data decodes to more than a part: `part at offset <offset> is longer than 4096
    /// bytes`. Nothing is stored.
    PartTooLong {
        /// The offset the part names.
        offset: u64,
    },
    /// The part is not at the next offset: `part at offset <offset>, the next offset is
    /// <next_offset>`. Nothing is stored.
    PartAtOffset {
        /// The offset the part names.
        offset: u64,
        /// The offset the upload expects.
        next_offset: usize,
    },
    /// A received part comes again with other bytes: `part at offset <offset> was received with
    /// other bytes`. The received bytes stay.
    PartOtherBytes {
        /// The offset the part names.
        offset: u64,
    },
    /// The part's CRC-32 does not match its bytes: `part at offset <offset> has the wrong crc32`.
    PartCrc {
        /// The offset the part names.
        offset: u64,
    },
    /// The part is not as long as a part at its offset is: `part at offset <offset> is <len>
    /// bytes, a part is <expected> bytes`.
    PartLength {
        /// The offset the part names.
        offset: u64,
        /// The part's length.
        len: usize,
        /// The length a part at this offset has.
        expected: usize,
    },
    /// The SHA-256 of the whole upload is not the one `transfer.begin` gave: `transfer sha256 does
    /// not match begin's`. The upload is dropped, so none of its parts can be used.
    Sha256Mismatch,
    /// The upload has not every byte yet: `transfer is receiving: next offset <next_offset> of
    /// <bytes> bytes`.
    NotComplete {
        /// The offset of the next byte.
        next_offset: usize,
        /// The upload's size.
        bytes: usize,
    },
    /// The get starts past the end of the held reply: `reply offset <offset> is past the reply of
    /// <bytes> bytes`.
    GetOffset {
        /// The offset asked for.
        offset: u64,
        /// The reply's length.
        bytes: usize,
    },
    /// The get asks for more than a part: `reply part of <len> bytes is above 4096 bytes`.
    GetLength {
        /// The length asked for.
        len: u64,
    },
}

impl fmt::Display for TransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(id) => write!(formatter, "transfer busy: {id}"),
            Self::TooLarge { bytes, max } => write!(
                formatter,
                "transfer too large: {bytes}, the device takes at most {max}"
            ),
            Self::Unknown => formatter.write_str("unknown transfer"),
            Self::ReplyTooLong { capacity } => {
                write!(formatter, "reply longer than {capacity} bytes")
            }
            Self::IdsUsedUp => formatter.write_str("transfer ids are used up until the next boot"),
            Self::NoRoom { bytes } => {
                write!(formatter, "no room for a transfer of {bytes} bytes")
            }
            Self::PartData { offset } => {
                write!(formatter, "part at offset {offset} is not canonical base64")
            }
            Self::PartTooLong { offset } => write!(
                formatter,
                "part at offset {offset} is longer than {PART_BYTES} bytes"
            ),
            Self::PartAtOffset {
                offset,
                next_offset,
            } => write!(
                formatter,
                "part at offset {offset}, the next offset is {next_offset}"
            ),
            Self::PartOtherBytes { offset } => {
                write!(
                    formatter,
                    "part at offset {offset} was received with other bytes"
                )
            }
            Self::PartCrc { offset } => {
                write!(formatter, "part at offset {offset} has the wrong crc32")
            }
            Self::PartLength {
                offset,
                len,
                expected,
            } => write!(
                formatter,
                "part at offset {offset} is {len} bytes, a part is {expected} bytes"
            ),
            Self::Sha256Mismatch => formatter.write_str("transfer sha256 does not match begin's"),
            Self::NotComplete { next_offset, bytes } => write!(
                formatter,
                "transfer is receiving: next offset {next_offset} of {bytes} bytes"
            ),
            Self::GetOffset { offset, bytes } => write!(
                formatter,
                "reply offset {offset} is past the reply of {bytes} bytes"
            ),
            Self::GetLength { len } => {
                write!(
                    formatter,
                    "reply part of {len} bytes is above {PART_BYTES} bytes"
                )
            }
        }
    }
}

impl core::error::Error for TransferError {}

/// Whether an upload has every byte yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Bytes are still missing.
    Receiving,
    /// Every byte is received and the SHA-256 matches.
    Complete,
}

/// The state of the open upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Whether every byte is received.
    pub state: State,
    /// The offset of the next byte the upload takes.
    pub next_offset: usize,
    /// The upload's size, in bytes.
    pub bytes: usize,
}

/// The head of a held reply: the id that names it, its length and its SHA-256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyHead {
    /// The id the client reads the reply by.
    pub transfer: TransferId,
    /// The reply's length, in bytes.
    pub bytes: usize,
    /// The SHA-256 of the whole reply.
    pub sha256: [u8; 32],
}

/// One part of a held reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyPart<'a> {
    /// The offset of the first byte of `data` in the reply.
    pub offset: usize,
    /// The bytes from `offset`: at most [`PART_BYTES`], fewer at the end of the reply.
    pub data: &'a [u8],
    /// The CRC-32 of `data`.
    pub crc32: u32,
}

/// Reserves a zeroed buffer of the given length, or refuses when the heap cannot hold it. The
/// session takes this function, so that a test can make a reservation fail.
type Reserve = fn(usize) -> Result<Vec<u8>, TryReserveError>;

/// Reserves `len` zeroed bytes with `try_reserve_exact`: a heap that cannot hold them is an error
/// that the caller refuses, never an abort (specification 34, PORT-05).
fn reserve_zeroed(len: usize) -> Result<Vec<u8>, TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

/// The transfers of one boot: at most one open upload and at most one held reply. A new boot
/// starts a new session, so a reset ends every transfer (specification 34, PORT-05).
#[derive(Debug)]
pub struct TransferSession {
    ids: IdSequence,
    max_bytes: usize,
    reply_capacity: usize,
    reserve: Reserve,
    upload: Option<Upload>,
    reply: Option<HeldReply>,
}

/// An open upload. Both buffers are reserved at begin and freed with the upload. `buffer` holds
/// the whole container, and its bytes before `next_offset` are received. `part` is where each part
/// is decoded before its bytes are copied into `buffer`.
#[derive(Debug)]
struct Upload {
    id: TransferId,
    digest: [u8; 32],
    buffer: Vec<u8>,
    part: Vec<u8>,
    next_offset: usize,
}

impl Upload {
    fn state(&self) -> State {
        if self.next_offset == self.buffer.len() {
            State::Complete
        } else {
            State::Receiving
        }
    }
}

/// A reply above the frame, kept whole until it is read.
#[derive(Debug)]
struct HeldReply {
    id: TransferId,
    bytes: Vec<u8>,
}

impl TransferSession {
    /// A session of `boot` that takes an upload of at most `max_bytes` and holds a reply of at most
    /// `reply_capacity` bytes. Its buffers come from the heap.
    #[must_use]
    pub const fn new(boot: u32, max_bytes: usize, reply_capacity: usize) -> Self {
        Self::with_reserve(boot, max_bytes, reply_capacity, reserve_zeroed)
    }

    /// As [`Self::new`], with the buffers of an upload reserved by `reserve`. A test passes a
    /// function that refuses, to see that begin refuses rather than aborts.
    #[must_use]
    pub(crate) const fn with_reserve(
        boot: u32,
        max_bytes: usize,
        reply_capacity: usize,
        reserve: Reserve,
    ) -> Self {
        Self {
            ids: IdSequence::new(boot),
            max_bytes,
            reply_capacity,
            reserve,
            upload: None,
            reply: None,
        }
    }

    /// Opens an upload of `bytes` bytes whose SHA-256 is `digest`, and returns its id.
    ///
    /// The size is checked against the maximum before anything is reserved. Then the upload's
    /// buffer and a part buffer of [`PART_BYTES`] are reserved. These are the only buffers a
    /// transfer reserves, so a part allocates nothing.
    ///
    /// An upload of no bytes is complete at once, so its digest is checked here.
    ///
    /// # Errors
    ///
    /// [`TransferError::Busy`] while another upload is open, [`TransferError::TooLarge`] above the
    /// maximum, [`TransferError::NoRoom`] when the heap cannot hold the buffers,
    /// [`TransferError::IdsUsedUp`] when the boot has no ids left, and
    /// [`TransferError::Sha256Mismatch`] for an empty upload whose digest is not that of no bytes.
    pub fn begin(&mut self, bytes: u64, digest: [u8; 32]) -> Result<TransferId, TransferError> {
        if let Some(open) = &self.upload {
            return Err(TransferError::Busy(open.id));
        }
        let size = usize::try_from(bytes)
            .ok()
            .filter(|size| *size <= self.max_bytes)
            .ok_or(TransferError::TooLarge {
                bytes,
                max: self.max_bytes,
            })?;
        let buffer = (self.reserve)(size).map_err(|_| TransferError::NoRoom { bytes })?;
        let part = (self.reserve)(PART_BYTES).map_err(|_| TransferError::NoRoom { bytes })?;
        let id = self.ids.next_id().ok_or(TransferError::IdsUsedUp)?;
        let upload = Upload {
            id,
            digest,
            buffer,
            part,
            next_offset: 0,
        };
        if upload.state() == State::Complete && sha256(&upload.buffer) != upload.digest {
            return Err(TransferError::Sha256Mismatch);
        }
        self.upload = Some(upload);
        Ok(id)
    }

    /// Decodes the part at `offset` of the open upload `id` from its standard base64 `encoded`,
    /// checks that `crc` is the CRC-32 of its bytes, stores it when it is the next part, and returns
    /// the next offset.
    ///
    /// The bytes are decoded into the part buffer that begin reserved, and the CRC-32 is checked
    /// there before anything is copied into the upload. A part at the next offset is stored; it is
    /// as long as a part at its offset is. A repeat of a received part is acknowledged again when
    /// its bytes are the received ones, and refused when they are other bytes. When the last byte
    /// arrives, the SHA-256 of the upload is compared with the one begin gave, and a mismatch drops
    /// the upload with its buffers.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload, [`TransferError::PartData`]
    /// when `encoded` is not canonical base64, [`TransferError::PartTooLong`] when it decodes to
    /// more than a part, [`TransferError::PartCrc`] when the CRC-32 does not match,
    /// [`TransferError::PartAtOffset`] for any offset that is neither the next one nor a received
    /// part, [`TransferError::PartOtherBytes`] for a repeat with other bytes,
    /// [`TransferError::PartLength`] for a part of the wrong length, and
    /// [`TransferError::Sha256Mismatch`] when the completed upload is not the one begin named.
    pub fn part(
        &mut self,
        id: TransferId,
        offset: u64,
        encoded: &str,
        crc: u32,
    ) -> Result<usize, TransferError> {
        let upload = self.open_upload_mut(id)?;
        let decoded = STANDARD
            .decode_slice(encoded, &mut upload.part[..])
            .map_err(|error| match error {
                DecodeSliceError::DecodeError(_) => TransferError::PartData { offset },
                DecodeSliceError::OutputSliceTooSmall => TransferError::PartTooLong { offset },
            })?;
        if crc32::checksum(&upload.part[..decoded]) != crc {
            return Err(TransferError::PartCrc { offset });
        }
        let received = upload.next_offset;
        let bytes = upload.buffer.len();
        let start = usize::try_from(offset).ok();
        if start == Some(received) && received < bytes {
            let expected = PART_BYTES.min(bytes - received);
            if decoded != expected {
                return Err(TransferError::PartLength {
                    offset,
                    len: decoded,
                    expected,
                });
            }
            let end = received + expected;
            upload.buffer[received..end].copy_from_slice(&upload.part[..decoded]);
            upload.next_offset = end;
            if end == bytes && sha256(&upload.buffer) != upload.digest {
                self.upload = None;
                return Err(TransferError::Sha256Mismatch);
            }
            return Ok(end);
        }
        let repeat = start.filter(|start| *start < received && start.is_multiple_of(PART_BYTES));
        let Some(start) = repeat else {
            return Err(TransferError::PartAtOffset {
                offset,
                next_offset: received,
            });
        };
        let expected = PART_BYTES.min(bytes - start);
        if decoded == expected && upload.part[..decoded] == upload.buffer[start..start + expected] {
            Ok(received)
        } else {
            Err(TransferError::PartOtherBytes { offset })
        }
    }

    /// The state of the open upload `id`, with its next offset and its size.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload.
    pub fn status(&self, id: TransferId) -> Result<Status, TransferError> {
        let Some(upload) = self.upload.as_ref().filter(|upload| upload.id == id) else {
            return Err(TransferError::Unknown);
        };
        Ok(Status {
            state: upload.state(),
            next_offset: upload.next_offset,
            bytes: upload.buffer.len(),
        })
    }

    /// Takes the bytes of the complete upload `id` and ends the upload, so the next begin is
    /// accepted. The part buffer is freed with the upload.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload and
    /// [`TransferError::NotComplete`] while bytes are still missing; the upload stays open then.
    pub fn take_complete(&mut self, id: TransferId) -> Result<Vec<u8>, TransferError> {
        let upload = self.open_upload_mut(id)?;
        if upload.state() != State::Complete {
            return Err(TransferError::NotComplete {
                next_offset: upload.next_offset,
                bytes: upload.buffer.len(),
            });
        }
        let complete = self.upload.take().ok_or(TransferError::Unknown)?;
        Ok(complete.buffer)
    }

    /// Frees the buffers and the id of the open upload `id`, or of the held reply `id`.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` names neither.
    pub fn abort(&mut self, id: TransferId) -> Result<(), TransferError> {
        if self.upload.as_ref().is_some_and(|upload| upload.id == id) {
            self.upload = None;
        } else if self.reply.as_ref().is_some_and(|held| held.id == id) {
            self.reply = None;
        } else {
            return Err(TransferError::Unknown);
        }
        Ok(())
    }

    /// Holds `reply`, a reply longer than the frame, whole under a new id, and returns its head.
    /// It replaces the reply held before, which its id then no longer names.
    ///
    /// The session reserves nothing for a reply: `reply` is the caller's buffer, moved in. The
    /// reply's text is allocated by its encoder, as every control reply's is, and never by the
    /// upload's buffers.
    ///
    /// # Errors
    ///
    /// [`TransferError::ReplyTooLong`] when `reply` is longer than the reply buffer, which changes
    /// nothing, and [`TransferError::IdsUsedUp`] when the boot has no ids left.
    pub fn hold_reply(&mut self, reply: Vec<u8>) -> Result<ReplyHead, TransferError> {
        if reply.len() > self.reply_capacity {
            return Err(TransferError::ReplyTooLong {
                capacity: self.reply_capacity,
            });
        }
        let transfer = self.ids.next_id().ok_or(TransferError::IdsUsedUp)?;
        let head = ReplyHead {
            transfer,
            bytes: reply.len(),
            sha256: sha256(&reply),
        };
        self.reply = Some(HeldReply {
            id: transfer,
            bytes: reply,
        });
        Ok(head)
    }

    /// Reads up to `len` bytes of the held reply `id` from `offset`, with their CRC-32. The data
    /// ends at the end of the reply, so the last part may be shorter.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the held reply, [`TransferError::GetLength`]
    /// when `len` is above [`PART_BYTES`], and [`TransferError::GetOffset`] when `offset` is past
    /// the end of the reply.
    pub fn get(
        &self,
        id: TransferId,
        offset: u64,
        len: u64,
    ) -> Result<ReplyPart<'_>, TransferError> {
        let Some(held) = self.reply.as_ref().filter(|held| held.id == id) else {
            return Err(TransferError::Unknown);
        };
        let wanted = usize::try_from(len)
            .ok()
            .filter(|wanted| *wanted <= PART_BYTES)
            .ok_or(TransferError::GetLength { len })?;
        let total = held.bytes.len();
        let start = usize::try_from(offset)
            .ok()
            .filter(|start| *start <= total)
            .ok_or(TransferError::GetOffset {
                offset,
                bytes: total,
            })?;
        let end = start.saturating_add(wanted).min(total);
        let data = &held.bytes[start..end];
        Ok(ReplyPart {
            offset: start,
            data,
            crc32: crc32::checksum(data),
        })
    }

    fn open_upload_mut(&mut self, id: TransferId) -> Result<&mut Upload, TransferError> {
        self.upload
            .as_mut()
            .filter(|upload| upload.id == id)
            .ok_or(TransferError::Unknown)
    }

    /// The capacity of the open upload's buffer. A test reads it to see that a refused begin kept
    /// nothing and that an abort freed the buffer.
    #[cfg(test)]
    pub(crate) fn buffer_capacity(&self) -> usize {
        self.upload
            .as_ref()
            .map_or(0, |upload| upload.buffer.capacity())
    }

    /// The capacity of the open upload's part buffer. A test reads it to see that every part is
    /// decoded into the buffer that begin reserved.
    #[cfg(test)]
    pub(crate) fn part_buffer_capacity(&self) -> usize {
        self.upload
            .as_ref()
            .map_or(0, |upload| upload.part.capacity())
    }
}
