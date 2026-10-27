//! A chip's pairings (specification 11 §6.9.6; specification 34 S5): the shared rules of
//! [`crate::access::pairing`] over a record of two flash sectors, `pairing_a` and `pairing_b` of the
//! chip file, written as the device settings are, and over the chip's hardware random number
//! generator, which is random only while the radio runs. The record holds the pairing file's JSON
//! without spaces, so a sector holds as many tokens as fit it ([`PairingPort::fits`]).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::access::pairing::{PairingFile, PairingPort};
use crate::flash_record::{RecordStore, SectorFlash, StoreError};

/// The magic of a pairing record, which a record of the device settings never has.
pub const MAGIC: [u8; 8] = *b"TRSTPR01";

/// The refusal of a chip that serves no network: without the radio its generator gives no random
/// numbers that a code or a token may be drawn from.
pub const NO_NETWORK: &str = "pairing is for clients on the network, and the device serves none";

/// The chip's random number generator: fills the bytes it is given.
pub type RandomSource = Box<dyn FnMut(&mut [u8])>;

/// The pairing port of a chip: its record and its random numbers.
pub struct ChipPairingPort {
    store: RecordStore,
    /// The newest record's text as the store found it at the boot, which the first load reads.
    stored: Option<Vec<u8>>,
    /// The generator while the chip serves a network, `None` otherwise.
    random: Option<RandomSource>,
}

impl ChipPairingPort {
    /// The port over the two sectors of `flash`, with the newest record they hold, and `random`, the
    /// generator of a chip that serves a network.
    #[must_use]
    pub fn open(flash: Box<dyn SectorFlash>, random: Option<RandomSource>) -> Self {
        let (store, found) = RecordStore::open(flash, MAGIC);
        Self {
            store,
            stored: found.map(|(_, text)| text),
            random,
        }
    }
}

impl PairingPort for ChipPairingPort {
    /// The record's pairing file; none for sectors without a record. A record whose CRC-32 holds
    /// but whose text is not a pairing file is an error, so that it grants nothing.
    fn load(&mut self) -> Result<PairingFile, String> {
        match self.stored.take() {
            None => Ok(PairingFile::default()),
            Some(text) => serde_json::from_slice(&text)
                .map_err(|error| format!("parse pairing record: {error}")),
        }
    }

    fn save(&mut self, file: &PairingFile) -> Result<(), String> {
        let text =
            serde_json::to_vec(file).map_err(|error| format!("write pairing record: {error}"))?;
        self.store
            .store(&text)
            .map(|_| ())
            .map_err(|error| store_error(error).to_string())
    }

    fn random(&mut self, bytes: &mut [u8]) -> Result<(), String> {
        let random = self.random.as_mut().ok_or_else(|| NO_NETWORK.to_string())?;
        random(bytes);
        Ok(())
    }

    fn fits(&self, file: &PairingFile) -> bool {
        serde_json::to_vec(file).is_ok_and(|text| text.len() <= self.store.capacity())
    }
}

/// The reply's text of a record that was not written.
fn store_error(error: StoreError) -> &'static str {
    match error {
        StoreError::TooLong => "the device has no room for another pairing",
        StoreError::Erase => "write pairing record: the flash erase failed",
        StoreError::Program => "write pairing record: the flash program failed",
        StoreError::Read => "write pairing record: the flash read-back failed",
        StoreError::ReadBack => "write pairing record: the flash did not read back as written",
    }
}

#[cfg(test)]
mod tests;
