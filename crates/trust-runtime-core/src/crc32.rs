//! CRC-32/ISO-HDLC (IEEE): reflected polynomial 0xEDB88320, initial and final
//! XOR 0xFFFFFFFF. This is integrity checking, not authentication.
//!
//! Hosted builds use the existing accelerated implementation. Bare-metal builds
//! use the same wire algorithm without crc32fast's 16 KiB slicing table. The
//! compact loop performs exactly eight bounded bit steps per input byte; CRC
//! checks belong to bounded artifact loading, not the timed PLC scan.

/// Calculate the STBC and stopped-install bundle wire checksum.
#[must_use]
pub fn checksum(bytes: &[u8]) -> u32 {
    #[cfg(feature = "std")]
    {
        crc32fast::hash(bytes)
    }
    #[cfg(not(feature = "std"))]
    {
        compact(bytes)
    }
}

/// Calculate `checksum` over `parts` as if they were concatenated, without allocating.
#[must_use]
pub fn checksum_parts(parts: &[&[u8]]) -> u32 {
    #[cfg(feature = "std")]
    {
        let mut hasher = crc32fast::Hasher::new();
        for part in parts {
            hasher.update(part);
        }
        hasher.finalize()
    }
    #[cfg(not(feature = "std"))]
    {
        compact_parts(parts)
    }
}

#[cfg(any(not(feature = "std"), test))]
fn compact(bytes: &[u8]) -> u32 {
    compact_parts(&[bytes])
}

#[cfg(any(not(feature = "std"), test))]
fn compact_parts(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for part in parts {
        for &byte in *part {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let polynomial = 0u32.wrapping_sub(crc & 1) & 0xedb8_8320;
                crc = (crc >> 1) ^ polynomial;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_matches_ieee_vectors_acceleration_and_real_artifacts() {
        assert_eq!(compact(b""), 0);
        assert_eq!(compact(b"123456789"), 0xcbf4_3926);
        let mut data = [0u8; 4097];
        let mut seed = 0x6d2b_79f5u32;
        for byte in &mut data {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
        // Every small extent and all alignment/table boundaries through 4096,
        // including deliberately unaligned slices and the byte on each side.
        for offset in 0..16 {
            for length in 0..=256 {
                let bytes = &data[offset..offset + length];
                assert_eq!(compact(bytes), crc32fast::hash(bytes));
                assert_eq!(checksum(bytes), compact(bytes));
            }
        }
        for length in [
            511, 512, 513, 1023, 1024, 1025, 2047, 2048, 2049, 4095, 4096, 4097,
        ] {
            assert_eq!(compact(&data[..length]), crc32fast::hash(&data[..length]));
        }
        for bytes in [
            &include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            )[..],
            &include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
            )[..],
        ] {
            assert_eq!(compact(bytes), crc32fast::hash(bytes));
            assert_eq!(checksum(bytes), compact(bytes));
            // The header checksum covers the section table and later payload.
            let start = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
            let expected = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!(compact(&bytes[start..]), expected);
        }
    }

    #[test]
    fn parts_match_the_checksum_of_their_concatenation() {
        let mut data = [0u8; 97];
        let mut seed = 0x9e37_79b9u32;
        for byte in &mut data {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
        let empty: &[u8] = &[];
        assert_eq!(checksum_parts(&[]), checksum(&[]));
        assert_eq!(checksum_parts(&[empty, empty]), checksum(&[]));
        for offset in 0..8 {
            for length in 0..=48 {
                let bytes = &data[offset..offset + length];
                let expected = checksum(bytes);
                let (head, rest) = bytes.split_at(length / 3);
                let (middle, tail) = rest.split_at(rest.len() / 2);
                let parts = [head, middle, tail];
                assert_eq!(checksum_parts(&parts), expected);
                assert_eq!(compact_parts(&parts), expected);
                // Empty leading and trailing parts leave the running state unchanged.
                assert_eq!(checksum_parts(&[empty, bytes, empty]), expected);
            }
        }
    }
}
