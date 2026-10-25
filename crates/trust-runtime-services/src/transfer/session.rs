//! The transfers of one boot (specification 34, PORT-05): one open upload, and one held reply, kept
//! in the parts it was written in and read back in ranges. An upload is held in memory until its last
//! byte arrives, or a chip writes it into its program store part by part (§5.1), and the session then
//! keeps only the container's header and the SHA-256 of the bytes written so far. An upload reserves
//! its buffers at begin, so a part allocates nothing. The rules here hold on every runtime; their JSON
//! shapes are in [`crate::transfer::wire`].

use alloc::borrow::Cow;
use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::{DecodeSliceError, Engine as _};
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{sha256, Sha256Stream};
use trust_runtime_core::native::image::MAGIC as IMAGE_MAGIC;
use trust_runtime_core::native::signature::{Prehash, SIGN_BYTES};

use super::ids::{IdSequence, TransferId};
use super::reply_parts::{reserve_part, PartReserve, ReplyParts};

/// The length of every part of an upload but the last, and the longest part a held reply is read
/// in (specification 34, PORT-05).
pub const PART_BYTES: usize = 4_096;

/// The bytes of a container's header, which the session keeps of an upload written into a program
/// store until the install programs it (specification 34 §5.1).
pub const HEADER_BYTES: usize = crate::installation::SEQUENCED_HEADER_BYTES;

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
    /// The heap had no room for the next part of a reply: `no room for a reply longer than
    /// <written> bytes`. The reply is refused whole, never cut and never an abort, and the parts
    /// already written are freed.
    NoRoomForReply {
        /// The bytes of the reply written when the heap had no room.
        written: usize,
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
    /// The upload is written into the program store, which only `program.install` takes and only
    /// the platform that writes the store serves: `the upload is written into the program store`.
    InStore,
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
            Self::NoRoomForReply { written } => {
                write!(formatter, "no room for a reply longer than {written} bytes")
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
            Self::InStore => formatter.write_str("the upload is written into the program store"),
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

/// What a part did to its upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Received {
    /// The part is stored, or it repeats a received part with the same bytes: the upload's next
    /// offset.
    Next(usize),
    /// The part is the next one of an upload written into a program store. Its bytes wait in the part
    /// buffer ([`TransferSession::part_bytes`]) until the platform has written them at `offset`, and
    /// [`TransferSession::written`] then takes them (specification 34 §5.1).
    Write {
        /// The offset of the part's first byte in the upload.
        offset: usize,
    },
    /// The part repeats the received part at `offset` of an upload written into a program store,
    /// with the length a part there has. The platform compares it with the bytes the store holds
    /// there through [`TransferSession::repeated`].
    Repeat {
        /// The offset of the part's first byte in the upload.
        offset: usize,
    },
}

/// A complete upload, as the operation that uses it takes it.
#[derive(Debug, PartialEq, Eq)]
pub enum Complete {
    /// An upload held in memory: its bytes, and when it is a program image, the SHA-512 of the bytes
    /// its signature covers, which the session built as the parts arrived (specification 44 §5.11).
    Memory(Vec<u8>, Option<Prehash>),
    /// An upload written into a program store.
    Store(StoredUpload),
}

/// A complete upload that a chip wrote into its program store (specification 34 §5.1): every byte
/// but the container's header lies in the store, and the session kept the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUpload {
    /// The id the upload had.
    pub transfer: TransferId,
    /// The upload's length, in bytes.
    pub bytes: usize,
    /// The upload's first bytes, up to [`HEADER_BYTES`]: the container's header, which the store
    /// does not hold.
    pub header: Vec<u8>,
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

/// One range of a held reply, as `transfer.get` answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyPart<'a> {
    /// The offset of the first byte of `data` in the reply.
    pub offset: usize,
    /// The bytes from `offset`: at most [`PART_BYTES`], fewer at the end of the reply. Borrowed from
    /// the held reply when the range lies in one of its parts, and copied when the range crosses two
    /// parts or reaches the last byte of the reply, which the get then frees.
    pub data: Cow<'a, [u8]>,
    /// The CRC-32 of `data`.
    pub crc32: u32,
}

impl<'a> ReplyPart<'a> {
    /// The range of `data` that starts at `offset`, with its CRC-32.
    fn of(offset: usize, data: Cow<'a, [u8]>) -> Self {
        let crc32 = crc32::checksum(&data);
        Self {
            offset,
            data,
            crc32,
        }
    }
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
    part_reserve: PartReserve,
    upload: Option<Upload>,
    reply: Option<HeldReply>,
}

/// An open upload. Its buffers are reserved at begin and freed with the upload. `part` is where each
/// part is decoded, and `decoded` is the length of the part it holds; the bytes before `next_offset`
/// are received.
#[derive(Debug)]
struct Upload {
    id: TransferId,
    digest: [u8; 32],
    part: Vec<u8>,
    decoded: usize,
    next_offset: usize,
    bytes: usize,
    held: Held,
    /// The signed bytes of an upload held in memory that is a program image, known from its first
    /// part.
    signed: Option<Signed>,
}

/// The bytes of an upload that a program image's signature covers, every byte before `SIGN`, the
/// image's last section, and their SHA-512, which the session builds as the parts arrive. A device
/// then checks the signature of an online change without hashing the image in one block while its
/// program runs (specification 44 §5.11).
#[derive(Debug)]
struct Signed {
    /// The end of the signed bytes: the upload's length less `SIGN`.
    end: usize,
    hash: Prehash,
}

impl Signed {
    /// The signed bytes of an upload of `bytes` bytes whose first part is `first`: those of a
    /// program image, which starts with the image's magic, and none of any other upload.
    fn of(first: &[u8], bytes: usize) -> Option<Self> {
        first.starts_with(&IMAGE_MAGIC).then_some(())?;
        Some(Self {
            end: bytes.checked_sub(SIGN_BYTES)?,
            hash: Prehash::new(),
        })
    }

    /// Adds the signed bytes of `part`, the part at `offset`.
    fn add(&mut self, offset: usize, part: &[u8]) {
        let end = self.end.min(offset + part.len());
        if offset < end {
            self.hash.update(&part[..end - offset]);
        }
    }
}

/// Where an upload's received bytes are (specification 34, PORT-05 and §5.1).
#[derive(Debug)]
enum Held {
    /// In memory: the whole upload, whose bytes before the next offset are received.
    Memory(Vec<u8>),
    /// In a program store, which the platform writes part by part.
    Store(InStore),
}

/// What the session keeps of an upload written into a program store: the container's header, which
/// the install programs, the SHA-256 of the bytes written so far, and the offset of the part that
/// waits in the part buffer for the platform.
#[derive(Debug)]
struct InStore {
    header: Vec<u8>,
    hash: Sha256Stream,
    waiting: Option<usize>,
}

impl Upload {
    fn state(&self) -> State {
        if self.next_offset == self.bytes {
            State::Complete
        } else {
            State::Receiving
        }
    }

    /// The length a part at `offset` has: a whole part, or the rest of the upload.
    fn part_len(&self, offset: usize) -> usize {
        PART_BYTES.min(self.bytes - offset)
    }
}

/// A reply above the frame, kept in the parts it was written in until it is read.
#[derive(Debug)]
struct HeldReply {
    id: TransferId,
    parts: ReplyParts,
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
            part_reserve: reserve_part,
            upload: None,
            reply: None,
        }
    }

    /// As `self`, with each part of a reply taken by `reserve`. A test passes a function that
    /// refuses after some number of parts, to see that a reply the heap has no room for is refused
    /// rather than aborted.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_part_reserve(mut self, reserve: PartReserve) -> Self {
        self.part_reserve = reserve;
        self
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
        let size = self.admit(bytes)?;
        let buffer = (self.reserve)(size).map_err(|_| TransferError::NoRoom { bytes })?;
        self.open(size, digest, Held::Memory(buffer))
    }

    /// Opens an upload of `bytes` bytes whose SHA-256 is `digest` that the platform writes into its
    /// program store part by part, and returns its id (specification 34 §5.1). The session reserves
    /// the part buffer and the container's header only, and works the SHA-256 out part by part.
    ///
    /// # Errors
    ///
    /// Those of [`Self::begin`], with [`TransferError::NoRoom`] when the heap cannot hold the part
    /// buffer and the header.
    pub fn begin_in_store(
        &mut self,
        bytes: u64,
        digest: [u8; 32],
    ) -> Result<TransferId, TransferError> {
        let size = self.admit(bytes)?;
        let mut header =
            (self.reserve)(size.min(HEADER_BYTES)).map_err(|_| TransferError::NoRoom { bytes })?;
        header.clear();
        self.open(
            size,
            digest,
            Held::Store(InStore {
                header,
                hash: Sha256Stream::new(),
                waiting: None,
            }),
        )
    }

    /// The size of an upload of `bytes` bytes, checked before anything is reserved.
    fn admit(&self, bytes: u64) -> Result<usize, TransferError> {
        if let Some(open) = &self.upload {
            return Err(TransferError::Busy(open.id));
        }
        usize::try_from(bytes)
            .ok()
            .filter(|size| *size <= self.max_bytes)
            .ok_or(TransferError::TooLarge {
                bytes,
                max: self.max_bytes,
            })
    }

    /// Opens the upload of `size` bytes held in `held`: reserves its part buffer and gives it an id.
    /// An upload of no bytes is complete at once, so its digest is checked here.
    fn open(
        &mut self,
        size: usize,
        digest: [u8; 32],
        held: Held,
    ) -> Result<TransferId, TransferError> {
        let bytes = size as u64;
        let part = (self.reserve)(PART_BYTES).map_err(|_| TransferError::NoRoom { bytes })?;
        let id = self.ids.next_id().ok_or(TransferError::IdsUsedUp)?;
        let upload = Upload {
            id,
            digest,
            part,
            decoded: 0,
            next_offset: 0,
            bytes: size,
            held,
            signed: None,
        };
        if upload.state() == State::Complete && sha256(&[]) != upload.digest {
            return Err(TransferError::Sha256Mismatch);
        }
        self.upload = Some(upload);
        Ok(id)
    }

    /// Decodes the part at `offset` of the open upload `id` from its standard base64 `encoded`,
    /// checks that `crc` is the CRC-32 of its bytes, and stores it when it is the next part.
    ///
    /// The bytes are decoded into the part buffer that begin reserved, and the CRC-32 is checked
    /// there before anything is copied into the upload. A part at the next offset is stored; it is
    /// as long as a part at its offset is. A repeat of a received part is acknowledged again when
    /// its bytes are the received ones, and refused when they are other bytes. When the last byte
    /// arrives, the SHA-256 of the upload is compared with the one begin gave, and a mismatch drops
    /// the upload with its buffers. The result is the next offset, [`Received::Next`].
    ///
    /// A part of an upload written into a program store is checked the same way, but the platform
    /// stores it: the next part is [`Received::Write`], which [`Self::written`] takes once the
    /// platform has written it, and a repeat of a received part is [`Received::Repeat`], which the
    /// platform compares with the store through [`Self::repeated`].
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
    ) -> Result<Received, TransferError> {
        let upload = self.open_upload_mut(id)?;
        if let Held::Store(stored) = &mut upload.held {
            stored.waiting = None;
        }
        let decoded = STANDARD
            .decode_slice(encoded, &mut upload.part[..])
            .map_err(|error| match error {
                DecodeSliceError::DecodeError(_) => TransferError::PartData { offset },
                DecodeSliceError::OutputSliceTooSmall => TransferError::PartTooLong { offset },
            })?;
        if crc32::checksum(&upload.part[..decoded]) != crc {
            return Err(TransferError::PartCrc { offset });
        }
        upload.decoded = decoded;
        let received = upload.next_offset;
        let start = usize::try_from(offset).ok();
        if start == Some(received) && received < upload.bytes {
            let expected = upload.part_len(received);
            if decoded != expected {
                return Err(TransferError::PartLength {
                    offset,
                    len: decoded,
                    expected,
                });
            }
            let end = received + expected;
            let buffer = match &mut upload.held {
                Held::Memory(buffer) => buffer,
                Held::Store(stored) => {
                    stored.waiting = Some(received);
                    return Ok(Received::Write { offset: received });
                }
            };
            buffer[received..end].copy_from_slice(&upload.part[..decoded]);
            if received == 0 {
                upload.signed = Signed::of(&buffer[..end], upload.bytes);
            }
            if let Some(signed) = &mut upload.signed {
                signed.add(received, &buffer[received..end]);
            }
            upload.next_offset = end;
            if end == upload.bytes && sha256(buffer) != upload.digest {
                self.upload = None;
                return Err(TransferError::Sha256Mismatch);
            }
            return Ok(Received::Next(end));
        }
        let repeat = start.filter(|start| *start < received && start.is_multiple_of(PART_BYTES));
        let Some(start) = repeat else {
            return Err(TransferError::PartAtOffset {
                offset,
                next_offset: received,
            });
        };
        let expected = upload.part_len(start);
        if decoded != expected {
            return Err(TransferError::PartOtherBytes { offset });
        }
        match &mut upload.held {
            Held::Memory(buffer) if upload.part[..decoded] == buffer[start..start + expected] => {
                Ok(Received::Next(received))
            }
            Held::Memory(_) => Err(TransferError::PartOtherBytes { offset }),
            Held::Store(stored) => {
                stored.waiting = Some(start);
                Ok(Received::Repeat { offset: start })
            }
        }
    }

    /// The bytes of the part that waits in the part buffer of the upload `id` for the platform: the
    /// next part to write ([`Received::Write`]), or a repeat to compare ([`Received::Repeat`]).
    /// `None` when no part of an upload written into a program store waits.
    #[must_use]
    pub fn part_bytes(&self, id: TransferId) -> Option<&[u8]> {
        let upload = self.upload.as_ref().filter(|upload| upload.id == id)?;
        match &upload.held {
            Held::Store(InStore {
                waiting: Some(_), ..
            }) => Some(&upload.part[..upload.decoded]),
            _ => None,
        }
    }

    /// Takes the part that waited for the platform as written into the program store, and returns
    /// the next offset (specification 34 §5.1). Its bytes are added to the SHA-256, and its first
    /// bytes are kept as the container's header when it starts before [`HEADER_BYTES`]. When it is
    /// the last part, the SHA-256 of the upload is compared with the one begin gave, and a mismatch
    /// drops the upload.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload or no part of it waits to be
    /// written, and [`TransferError::Sha256Mismatch`] when the completed upload is not the one begin
    /// named.
    pub fn written(&mut self, id: TransferId) -> Result<usize, TransferError> {
        let upload = self.open_upload_mut(id)?;
        let received = upload.next_offset;
        let end = received + upload.decoded;
        let Held::Store(stored) = &mut upload.held else {
            return Err(TransferError::Unknown);
        };
        if stored.waiting.take() != Some(received) {
            return Err(TransferError::Unknown);
        }
        let part = &upload.part[..upload.decoded];
        if received < HEADER_BYTES {
            let header_end = HEADER_BYTES.min(end);
            stored
                .header
                .extend_from_slice(&part[..header_end - received]);
        }
        stored.hash.update(part);
        upload.next_offset = end;
        if end == upload.bytes && core::mem::take(&mut stored.hash).finish() != upload.digest {
            self.upload = None;
            return Err(TransferError::Sha256Mismatch);
        }
        Ok(end)
    }

    /// Compares the repeat that waits in the part buffer of the upload `id` with `stored`, the bytes
    /// the program store holds at the part's offset, and returns the next offset when they are the
    /// same bytes. The bytes before [`HEADER_BYTES`], which the store does not hold, are compared
    /// with the header the session kept, whatever `stored` holds there.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload or no repeat of it waits, and
    /// [`TransferError::PartOtherBytes`] when the bytes differ; the received bytes stay.
    pub fn repeated(&mut self, id: TransferId, stored: &[u8]) -> Result<usize, TransferError> {
        let upload = self.open_upload_mut(id)?;
        let Held::Store(kept) = &mut upload.held else {
            return Err(TransferError::Unknown);
        };
        let Some(offset) = kept
            .waiting
            .take()
            .filter(|offset| *offset < upload.next_offset)
        else {
            return Err(TransferError::Unknown);
        };
        let part = &upload.part[..upload.decoded];
        let in_header = HEADER_BYTES.saturating_sub(offset).min(part.len());
        let header_same = in_header == 0
            || kept.header.get(offset..offset + in_header) == Some(&part[..in_header]);
        let same = header_same && stored.get(in_header..part.len()) == Some(&part[in_header..]);
        if same {
            Ok(upload.next_offset)
        } else {
            Err(TransferError::PartOtherBytes {
                offset: offset as u64,
            })
        }
    }

    /// Whether the open upload `id` is written into a program store.
    #[must_use]
    pub fn in_store(&self, id: TransferId) -> bool {
        self.upload
            .as_ref()
            .is_some_and(|upload| upload.id == id && matches!(upload.held, Held::Store(_)))
    }

    /// Ends the open upload when it is written into a program store, and returns its id: a `restart`
    /// that starts the resource ends it, so that no part of it is written while the program runs
    /// (specification 34, PORT-05). An upload held in memory stays open.
    pub fn end_store_upload(&mut self) -> Option<TransferId> {
        let id = self
            .upload
            .as_ref()
            .filter(|upload| matches!(upload.held, Held::Store(_)))?
            .id;
        self.upload = None;
        Some(id)
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
            bytes: upload.bytes,
        })
    }

    /// Takes the complete upload `id` and ends the upload, so the next begin is accepted: the bytes
    /// of an upload held in memory, or what the session kept of one written into a program store.
    /// The part buffer is freed with the upload.
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the open upload and
    /// [`TransferError::NotComplete`] while bytes are still missing; the upload stays open then.
    pub fn take_complete(&mut self, id: TransferId) -> Result<Complete, TransferError> {
        let upload = self.open_upload_mut(id)?;
        if upload.state() != State::Complete {
            return Err(TransferError::NotComplete {
                next_offset: upload.next_offset,
                bytes: upload.bytes,
            });
        }
        let complete = self.upload.take().ok_or(TransferError::Unknown)?;
        Ok(match complete.held {
            Held::Memory(buffer) => {
                Complete::Memory(buffer, complete.signed.map(|signed| signed.hash))
            }
            Held::Store(stored) => Complete::Store(StoredUpload {
                transfer: complete.id,
                bytes: complete.bytes,
                header: stored.header,
            }),
        })
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

    /// The capacity of the reply buffer: the longest reply the session holds (specification 34,
    /// PORT-05). A computer's is 64 MiB, and a chip's is the capacity of its heap.
    #[must_use]
    pub const fn reply_capacity(&self) -> usize {
        self.reply_capacity
    }

    /// How the session takes each part of a reply from the heap. The writer of a reply for this
    /// session takes its parts the same way, so that a reply the session holds was taken from the
    /// heap as the session's platform decides.
    #[must_use]
    pub(crate) const fn part_reserve(&self) -> PartReserve {
        self.part_reserve
    }

    /// Holds `reply`, a reply longer than the frame, in the parts it was written in under a new id,
    /// and returns its head. It replaces the reply held before, which its id then no longer names.
    ///
    /// The session reserves nothing for a reply: the parts are the writer's, moved in, and the
    /// SHA-256 of the head is worked out over them without joining them. They are freed when the
    /// reply is read to its last byte, aborted or replaced, and when the session is dropped at a
    /// reset (specification 34, PORT-05).
    ///
    /// # Errors
    ///
    /// [`TransferError::ReplyTooLong`] when `reply` is longer than the reply buffer, which changes
    /// nothing, and [`TransferError::IdsUsedUp`] when the boot has no ids left. The held reply
    /// stays in both, and `reply` is dropped.
    pub fn hold_reply(&mut self, reply: ReplyParts) -> Result<ReplyHead, TransferError> {
        if reply.len() > self.reply_capacity {
            return Err(TransferError::ReplyTooLong {
                capacity: self.reply_capacity,
            });
        }
        let transfer = self.ids.next_id().ok_or(TransferError::IdsUsedUp)?;
        let head = ReplyHead {
            transfer,
            bytes: reply.len(),
            sha256: reply.sha256(),
        };
        self.reply = Some(HeldReply {
            id: transfer,
            parts: reply,
        });
        Ok(head)
    }

    /// Reads up to `len` bytes of the held reply `id` from `offset`, with their CRC-32. The data
    /// ends at the end of the reply, so the last range may be shorter, and a range across two parts
    /// of the reply is read like any other.
    ///
    /// A get that reads the last byte of the reply frees the reply: its data is copied out first, so
    /// the answer is whole, and then every part is freed, and a later use of the id is
    /// [`TransferError::Unknown`]. A get at the end of the reply, which reads no byte, and a get
    /// before it leave the reply held, so a part can be read again (specification 34, PORT-05).
    ///
    /// # Errors
    ///
    /// [`TransferError::Unknown`] when `id` is not the held reply, [`TransferError::GetLength`]
    /// when `len` is above [`PART_BYTES`], and [`TransferError::GetOffset`] when `offset` is past
    /// the end of the reply.
    pub fn get(
        &mut self,
        id: TransferId,
        offset: u64,
        len: u64,
    ) -> Result<ReplyPart<'_>, TransferError> {
        let (start, end, last) = self.locate(id, offset, len)?;
        if last {
            let held = self.reply.take().ok_or(TransferError::Unknown)?;
            let data = held.parts.range(start, end).into_owned();
            return Ok(ReplyPart::of(start, Cow::Owned(data)));
        }
        let held = self.reply.as_ref().ok_or(TransferError::Unknown)?;
        Ok(ReplyPart::of(start, held.parts.range(start, end)))
    }

    /// The range `get` reads, as its start and end and whether it reads the last byte of the reply.
    fn locate(
        &self,
        id: TransferId,
        offset: u64,
        len: u64,
    ) -> Result<(usize, usize, bool), TransferError> {
        let Some(held) = self.reply.as_ref().filter(|held| held.id == id) else {
            return Err(TransferError::Unknown);
        };
        let wanted = usize::try_from(len)
            .ok()
            .filter(|wanted| *wanted <= PART_BYTES)
            .ok_or(TransferError::GetLength { len })?;
        let total = held.parts.len();
        let start = usize::try_from(offset)
            .ok()
            .filter(|start| *start <= total)
            .ok_or(TransferError::GetOffset {
                offset,
                bytes: total,
            })?;
        let end = start.saturating_add(wanted).min(total);
        Ok((start, end, end == total && end > start))
    }

    fn open_upload_mut(&mut self, id: TransferId) -> Result<&mut Upload, TransferError> {
        self.upload
            .as_mut()
            .filter(|upload| upload.id == id)
            .ok_or(TransferError::Unknown)
    }

    /// The capacity of the open upload's buffer: the whole upload's held in memory, or the header's
    /// of one written into a program store. A test reads it to see that a refused begin kept nothing
    /// and that an abort freed the buffer.
    #[cfg(test)]
    pub(crate) fn buffer_capacity(&self) -> usize {
        self.upload.as_ref().map_or(0, |upload| match &upload.held {
            Held::Memory(buffer) => buffer.capacity(),
            Held::Store(stored) => stored.header.capacity(),
        })
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
