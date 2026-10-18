//! The unique ID of an STM32 chip, as the words its serial number is printed from
//! (specification 34, D.4 and D.5).

/// The three little-endian words of the 96-bit unique ID at 0x1FFF_7A10, rebuilt from the fields
/// the HAL reads there: the X and Y wafer coordinates, the wafer number and the seven lot
/// characters. Missing lot bytes read as zero.
pub fn uid_words(x: u16, y: u16, wafer: u8, lot: &[u8]) -> [u32; 3] {
    let lot_byte = |index: usize| lot.get(index).copied().unwrap_or(0);
    [
        u32::from(x) | (u32::from(y) << 16),
        u32::from_le_bytes([wafer, lot_byte(0), lot_byte(1), lot_byte(2)]),
        u32::from_le_bytes([lot_byte(3), lot_byte(4), lot_byte(5), lot_byte(6)]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unique_id_words_are_rebuilt_from_the_fields() {
        // Spec 34 D.4: UID 0x0034002D 0x34314715 0x35363630 on the owner's STM32F407G-DISC1:
        // X 0x2D, Y 0x34, wafer 0x15, lot "G140665".
        assert_eq!(
            uid_words(0x002d, 0x0034, 0x15, b"G140665"),
            [0x0034_002d, 0x3431_4715, 0x3536_3630]
        );
        // A short lot string reads as zeros, never out of bounds.
        assert_eq!(uid_words(1, 2, 3, b"AB"), [0x0002_0001, 0x0042_4103, 0]);
        assert_eq!(uid_words(0, 0, 0, b""), [0, 0, 0]);
        assert_eq!(
            uid_words(u16::MAX, u16::MAX, u8::MAX, b"\xff\xff\xff\xff\xff\xff\xff"),
            [u32::MAX; 3]
        );
    }
}
