//! The pre-shared key of a WPA2-Personal network from its passphrase (IEEE 802.11-2020 J.4.1:
//! PBKDF2 with HMAC-SHA1, the SSID as the salt, 4,096 iterations, 256 bits). A chip derives it
//! once at boot, before its program starts, and gives its radio the key (specification 34
//! PORT-06): the ESP32-C6's radio driver derived it at the first join, at a priority above the
//! program, and stalled the scans for about 0.85 seconds.

use alloc::string::String;
use core::fmt::Write;

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

/// The iterations of the derivation (IEEE 802.11-2020 J.4.1).
const ITERATIONS: u32 = 4_096;

/// The 256-bit pre-shared key of `passphrase` on the network `ssid`; none only if HMAC refused the
/// passphrase as its key, which HMAC, taking a key of any length, never does.
#[must_use]
pub fn psk(passphrase: &str, ssid: &str) -> Option<[u8; 32]> {
    let prf = Hmac::<Sha1>::new_from_slice(passphrase.as_bytes()).ok()?;
    let mut key = [0u8; 32];
    for (index, chunk) in (1u32..).zip(key.chunks_mut(20)) {
        let mut mac = prf.clone();
        mac.update(ssid.as_bytes());
        mac.update(&index.to_be_bytes());
        let mut block = mac.finalize().into_bytes();
        let mut sum = block;
        for _ in 1..ITERATIONS {
            let mut mac = prf.clone();
            mac.update(&block);
            block = mac.finalize().into_bytes();
            for (sum, byte) in sum.iter_mut().zip(block.iter()) {
                *sum ^= byte;
            }
        }
        chunk.copy_from_slice(&sum[..chunk.len()]);
    }
    Some(key)
}

/// The key of [`psk`] as the 64 hexadecimal digits a station takes in place of the passphrase.
#[must_use]
pub fn psk_hex(passphrase: &str, ssid: &str) -> Option<String> {
    let mut hex = String::with_capacity(64);
    for byte in psk(passphrase, ssid)? {
        let _ = write!(hex, "{byte:02x}");
    }
    Some(hex)
}

#[cfg(test)]
mod tests {
    use super::psk_hex;

    /// The test vectors of IEEE 802.11-2020 J.4.2. Catches a derivation with the wrong salt,
    /// iteration count or block order, whose key no access point accepts.
    #[test]
    fn the_keys_of_the_standards_test_vectors() {
        assert_eq!(
            psk_hex("password", "IEEE").as_deref(),
            Some("f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e")
        );
        assert_eq!(
            psk_hex("ThisIsAPassword", "ThisIsASSID").as_deref(),
            Some("0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af")
        );
    }
}
