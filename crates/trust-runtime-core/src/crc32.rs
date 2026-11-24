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

/// The [`checksum`] of bytes that arrive in pieces: the CRC-32 of their concatenation, worked out
/// piece by piece without joining them. A chip serves a cycle trace part by part from the records
/// it holds, with no whole file to check at once (specification 48 RU-59, "Memory").
#[derive(Clone)]
pub struct Crc32Stream {
    #[cfg(feature = "std")]
    hasher: crc32fast::Hasher,
    #[cfg(not(feature = "std"))]
    state: u32,
}

impl Default for Crc32Stream {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32Stream {
    /// The checksum of no bytes yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            hasher: crc32fast::Hasher::new(),
            #[cfg(not(feature = "std"))]
            state: u32::MAX,
        }
    }

    /// Adds `bytes` after the bytes added before.
    pub fn update(&mut self, bytes: &[u8]) {
        #[cfg(feature = "std")]
        self.hasher.update(bytes);
        #[cfg(not(feature = "std"))]
        {
            self.state = compact_update(self.state, bytes);
        }
    }

    /// The checksum of every byte added, in the order added.
    #[must_use]
    pub fn finish(self) -> u32 {
        #[cfg(feature = "std")]
        {
            self.hasher.finalize()
        }
        #[cfg(not(feature = "std"))]
        {
            !self.state
        }
    }
}

impl core::fmt::Debug for Crc32Stream {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Crc32Stream")
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
        crc = compact_update(crc, part);
    }
    !crc
}

/// The compact loop's state after `bytes`, from the state `crc` (before the final XOR).
#[cfg(any(not(feature = "std"), test))]
fn compact_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let polynomial = 0u32.wrapping_sub(crc & 1) & 0xedb8_8320;
            crc = (crc >> 1) ^ polynomial;
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_gives_the_checksum_of_its_pieces_joined() {
        // The IEEE check value: "123456789" sums to 0xCBF43926, however it is split, and the
        // compact loop's state carries from piece to piece the same way. Worked by hand for each
        // split point. Catches a stream that restarts at each piece, that applies the final XOR
        // per piece, or that drops an empty piece's place.
        assert_eq!(Crc32Stream::new().finish(), 0);
        let data = b"123456789";
        for split in 0..=data.len() {
            let mut stream = Crc32Stream::new();
            stream.update(&data[..split]);
            stream.update(&[]);
            stream.update(&data[split..]);
            assert_eq!(stream.finish(), 0xcbf4_3926, "split at {split}");
            let state = compact_update(compact_update(u32::MAX, &data[..split]), &data[split..]);
            assert_eq!(!state, 0xcbf4_3926, "compact split at {split}");
        }
    }

    #[test]
    fn compact_matches_ieee_vectors_and_acceleration() {
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
