//! A chip's pairings over its record (specification 11 §6.9.6; specification 34 Appendix D.2), on
//! two sectors of 4,096 bytes as the ESP32-C6's are. Every expected value is worked out here.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::retain::checkpoint::FlashError;

use super::*;
use crate::access::pairing::{PairingError, Pairings};
use crate::access::AccessRole;
use crate::flash_record;

/// A clock second with ten digits, as every second from 2001 to 2286 has: 2026-10-16 00:00:00 UTC.
const NOW: u64 = 1_792_108_800;

/// Two erased sectors of 4,096 bytes, shared with the test so that it can read them after a write.
#[derive(Clone)]
struct Sectors(Rc<RefCell<[Vec<u8>; 2]>>);

impl Sectors {
    fn erased() -> Self {
        Self(Rc::new(RefCell::new([
            vec![0xFF; 4_096],
            vec![0xFF; 4_096],
        ])))
    }
}

impl SectorFlash for Sectors {
    fn sector_bytes(&self) -> usize {
        4_096
    }

    fn read(&mut self, sector: usize, bytes: &mut [u8]) -> Result<(), FlashError> {
        bytes.copy_from_slice(&self.0.borrow()[sector][..bytes.len()]);
        Ok(())
    }

    fn erase(&mut self, sector: usize) -> Result<(), FlashError> {
        self.0.borrow_mut()[sector].fill(0xFF);
        Ok(())
    }

    fn program(&mut self, sector: usize, bytes: &[u8]) -> Result<(), FlashError> {
        self.0.borrow_mut()[sector][..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

/// A generator that fills each request with one byte, 1 for the first request, 2 for the next and
/// so on, so that every code and every token differs from the one before.
fn counting() -> RandomSource {
    let mut next = 0_u8;
    Box::new(move |bytes: &mut [u8]| {
        next = next.wrapping_add(1);
        bytes.fill(next);
    })
}

/// The pairings of a chip that serves a network, over `flash`.
fn pairings(flash: &Sectors) -> Pairings<ChipPairingPort> {
    let port = ChipPairingPort::open(Box::new(flash.clone()), Some(counting()));
    Pairings::open(port, NOW)
}

/// Starts a code and claims it at once; returns the token.
fn pair(pairings: &mut Pairings<ChipPairingPort>) -> Result<String, PairingError> {
    let code = pairings.start(NOW).expect("start").code;
    pairings.claim(&code, None, NOW)
}

/// A claim is written as the pairing file's JSON without spaces in a record under `TRSTPR01`, and
/// a chip that boots again over the same sectors grants the token its role. Catches a record under
/// the device settings' magic, which the settings would read, pretty JSON, which would hold fewer
/// tokens, and pairings that a reset loses.
#[test]
fn a_claim_is_kept_in_the_record_across_a_boot() {
    let flash = Sectors::erased();
    let mut first = pairings(&flash);
    let token = pair(&mut first).expect("a claim");

    let sectors = flash.0.borrow();
    let (sequence, text) = flash_record::decode(&MAGIC, &sectors[0]).expect("a pairing record");
    assert_eq!(sequence, 1);
    let text = core::str::from_utf8(text).expect("UTF-8");
    assert!(
        text.starts_with(r#"{"tokens":[{"id":"pair-1792108800-"#),
        "{text}"
    );
    assert!(!text.contains(' '), "no spaces: {text}");
    assert_eq!(flash_record::decode(b"TRSTDS01", &sectors[0]), None);
    drop(sectors);

    let mut again = pairings(&flash);
    assert_eq!(again.validate(&token, NOW), Some(AccessRole::Operator));
}

/// A sector of 4,096 bytes less the record's 20-byte header holds 4,076 bytes of JSON. A token row
/// is 168 bytes: `{"id":"pair-1792108800-XXXXXXXX",` (33), `"token":"` and its 43 characters and
/// `",` (54), `"created_at":1792108800,` (24), `"enabled":true,` (15), `"role":"operator",` (18) and
/// `"expires_at":1794700800}` (24). n rows take 11 + 168n + (n - 1) + 2 = 12 + 169n bytes: 24 rows
/// take 4,068 and fit, 25 would take 4,237 and do not. So the 25th claim is refused with `the device
/// has no room for another pairing`; after a revoke (4,069 bytes: `false` is one byte longer) the
/// next claim removes the revoked row and fits again. Catches a chip that writes past its sector, one
/// that refuses while a revoked token could make room, and a limit counted in another unit.
#[test]
fn a_sector_holds_24_tokens_and_a_revoked_one_makes_room() {
    let flash = Sectors::erased();
    let mut pairings = pairings(&flash);
    for claim in 1..=24 {
        assert!(pair(&mut pairings).is_ok(), "claim {claim}");
    }
    assert_eq!(pair(&mut pairings), Err(PairingError::NoRoom));
    let first = pairings.list(NOW)[0].id.clone();
    assert_eq!(pairings.revoke(&first, NOW), Ok(true));
    assert!(pair(&mut pairings).is_ok(), "the revoked token made room");
    let rows = pairings.list(NOW);
    assert_eq!(rows.len(), 24);
    assert!(rows.iter().all(|row| row.enabled && row.id != first));
}

/// A chip that serves no network has no random numbers to draw a code from, and refuses
/// `pair.start` with `pairing is for clients on the network, and the device serves none`; nothing is
/// written. Catches a code drawn from a generator that is not random without the radio.
#[test]
fn a_chip_without_a_network_refuses_to_pair() {
    let flash = Sectors::erased();
    let port = ChipPairingPort::open(Box::new(flash.clone()), None);
    let mut pairings = Pairings::open(port, NOW);
    let refused = pairings.start(NOW).expect_err("no network, no code");
    assert_eq!(
        refused.to_string(),
        "pairing is for clients on the network, and the device serves none"
    );
    assert!(flash
        .0
        .borrow()
        .iter()
        .all(|sector| sector.iter().all(|byte| *byte == 0xFF)));
}
