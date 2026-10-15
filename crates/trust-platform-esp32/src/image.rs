//! Identity fields of the running ESP application image header (the factory partition).

/// Bytes at the start of the running image that carry the fields this crate reads.
pub const IMAGE_HEADER_BYTES: usize = 24;
const IMAGE_MAGIC: u8 = 0xE9;

/// Identity fields declared by the running image header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeader {
    /// Little-endian chip ID at header offset 12 (13 on ESP32-C6).
    pub chip_id: u16,
    /// Flash size in KiB from the high nibble of header byte 3.
    pub flash_kib: u16,
}

/// Decode the first `IMAGE_HEADER_BYTES` (24) bytes of an ESP image. Returns `None` when the magic byte
/// is missing or the flash-size code is not one of 1, 2, 4, 8 or 16 MB.
pub fn decode_image_header(bytes: &[u8; IMAGE_HEADER_BYTES]) -> Option<ImageHeader> {
    if bytes[0] != IMAGE_MAGIC {
        return None;
    }
    let flash_kib = match bytes[3] >> 4 {
        0 => 1024,
        1 => 2048,
        2 => 4096,
        3 => 8192,
        4 => 16384,
        _ => return None,
    };
    Some(ImageHeader {
        chip_id: u16::from_le_bytes([bytes[12], bytes[13]]),
        flash_kib,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// First 24 bytes of the factory image measured by the M0 probe on the owned board.
    const PROBE_HEADER: [u8; IMAGE_HEADER_BYTES] = [
        0xe9, 0x04, 0x02, 0x30, 0x72, 0x00, 0x01, 0x42, 0xee, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x00,
        0x00, 0x00, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];

    #[test]
    fn measured_header_reports_chip_13_and_8_mb() {
        assert_eq!(
            decode_image_header(&PROBE_HEADER),
            Some(ImageHeader {
                chip_id: 13,
                flash_kib: 8192,
            })
        );
    }

    #[test]
    fn every_flash_size_code_maps_to_its_capacity() {
        for (code, kib) in [(0u8, 1024u16), (1, 2048), (2, 4096), (3, 8192), (4, 16384)] {
            let mut header = PROBE_HEADER;
            header[3] = (code << 4) | 0x02;
            assert_eq!(decode_image_header(&header).map(|h| h.flash_kib), Some(kib));
        }
        for code in 5u8..=15 {
            let mut header = PROBE_HEADER;
            header[3] = code << 4;
            assert_eq!(decode_image_header(&header), None);
        }
    }

    #[test]
    fn missing_magic_is_rejected_and_chip_id_is_little_endian() {
        let mut header = PROBE_HEADER;
        header[0] = 0x00;
        assert_eq!(decode_image_header(&header), None);
        let mut header = PROBE_HEADER;
        header[12] = 0x34;
        header[13] = 0x12;
        assert_eq!(
            decode_image_header(&header).map(|h| h.chip_id),
            Some(0x1234)
        );
    }
}
