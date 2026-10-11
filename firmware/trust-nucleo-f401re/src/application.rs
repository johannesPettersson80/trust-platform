//! Independent, stopped-install application sector. No artifact is linked into firmware.
#[cfg(all(target_arch = "arm", target_os = "none"))]
const ADDRESS: usize = 0x0800_4000;
const CAPACITY: usize = 16 * 1024;
const HEADER_BYTES: usize = 32;

pub struct Applications<'a> {
    pub main: &'a [u8],
    pub numeric: &'a [u8],
    pub gpio: &'a [u8],
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn load() -> Result<Applications<'static>, &'static str> {
    // SAFETY: F401RE flash sector 1 is readable, independently installed before
    // reset, never programmed by this firmware, and excluded by the linker.
    let sector = unsafe { core::slice::from_raw_parts(ADDRESS as *const u8, CAPACITY) };
    decode(sector)
}

/// Validate the stopped-install bundle before exposing any application payload.
pub fn decode(sector: &[u8]) -> Result<Applications<'_>, &'static str> {
    if sector.len() < HEADER_BYTES || sector.len() > CAPACITY {
        return Err("bundle-size");
    }
    if &sector[..8] != b"TRSTB002" {
        return Err("bundle-magic");
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            sector[offset],
            sector[offset + 1],
            sector[offset + 2],
            sector[offset + 3],
        ])
    };
    let mut position = HEADER_BYTES;
    let mut payloads = [&[][..]; 3];
    for (index, payload) in payloads.iter_mut().enumerate() {
        let length = word(8 + index * 4) as usize;
        if length == 0 {
            return Err("bundle-empty");
        }
        let end = position.checked_add(length).ok_or("bundle-range")?;
        *payload = sector.get(position..end).ok_or("bundle-range")?;
        if trust_runtime_core::crc32::checksum(payload) != word(20 + index * 4) {
            return Err("bundle-crc");
        }
        position = end;
    }
    Ok(Applications {
        main: payloads[0],
        numeric: payloads[1],
        gpio: payloads[2],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> [u8; 35] {
        let mut bytes = [0u8; 35];
        bytes[..8].copy_from_slice(b"TRSTB002");
        for index in 0..3 {
            bytes[8 + index * 4..12 + index * 4].copy_from_slice(&1u32.to_le_bytes());
            let value = index as u8 + 1;
            bytes[20 + index * 4..24 + index * 4]
                .copy_from_slice(&trust_runtime_core::crc32::checksum(&[value]).to_le_bytes());
            bytes[32 + index] = value;
        }
        bytes
    }

    #[test]
    fn independent_payloads_are_bounded_and_checked_before_exposure() {
        let bytes = bundle();
        let result = decode(&bytes).unwrap();
        assert_eq!(result.main, &[1]);
        assert_eq!(result.numeric, &[2]);
        assert_eq!(result.gpio, &[3]);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err());
        }
        let mut corrupt = bytes;
        corrupt[34] ^= 1;
        assert!(matches!(decode(&corrupt), Err("bundle-crc")));
        let mut huge = bytes;
        huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(decode(&huge), Err("bundle-range")));
        let mut empty = bytes;
        empty[8..12].fill(0);
        assert!(matches!(decode(&empty), Err("bundle-empty")));
        let mut foreign = bytes;
        foreign[7] = b'1';
        assert!(matches!(decode(&foreign), Err("bundle-magic")));
    }
}
