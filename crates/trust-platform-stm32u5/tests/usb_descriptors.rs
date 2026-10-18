//! The USB identity and the peripheral's constants of the control link (design §2.1, §2.2 and §2.5;
//! specification 34, PORT-05). The expected values come from the specification, the design and the
//! bench record the design cites, written here as literals.

use trust_platform_stm32u5::usb::{
    bcd_device, serial_text, OTG_ENDPOINTS, OTG_FIFO_WORDS, OTG_HS_BASE,
};

/// The serial number is the chip's 96-bit unique ID: its three words in address order, each as eight
/// uppercase hexadecimal digits, most significant first (design §2.5, S1b §2.4). The bench F407 of
/// specification 34 D.4 reads `0034002D3431471535363630` for its words. The wrong behaviour: the
/// words in another order, lowercase digits, or a word printed without its leading zeros.
#[test]
fn the_serial_number_is_the_unique_id_as_24_uppercase_hex_digits() {
    assert_eq!(
        serial_text([0x0034_002D, 0x3431_4715, 0x3536_3630]),
        *b"0034002D3431471535363630"
    );
    assert_eq!(
        serial_text([0x0034_002D, 0x3431_4715, 0x0000_000A]),
        *b"0034002D343147150000000A"
    );
    assert_eq!(
        serial_text([0xABCD_EF01, 0, 0]),
        *b"ABCDEF010000000000000000"
    );
}

/// The bcdDevice is the major version in the high byte and the minor version in the low byte, each
/// in binary-coded decimal (USB 2.0 §9.6.1). truST 0.24 reads 0x0024, not the 0x0018 of decimal 24.
/// The wrong behaviour: the minor version read as a binary number, a patch number in the value, or a
/// version whose number does not fit two BCD digits accepted with its digits lost.
#[test]
fn the_bcd_device_is_the_major_and_minor_version_in_bcd() {
    assert_eq!(bcd_device("0.24.75"), Some(0x0024));
    assert_eq!(bcd_device("1.2.0"), Some(0x0102));
    assert_eq!(bcd_device("0.99"), Some(0x0099));
    assert_eq!(bcd_device("0.100.0"), None);
    assert_eq!(bcd_device("0.x.0"), None);
    assert_eq!(bcd_device("1"), None);
}

/// The OTG_HS register block is at 0x4204_0000, with 4 KiB of FIFO RAM (1024 words) and nine device
/// endpoints (design §2.1 and §2.2; DS13543 §3.55; the PAC's `DIEPCTL0` to `DIEPCTL8`). The wrong
/// behaviour: a register block at another address, a FIFO counted in bytes, or a missing endpoint.
#[test]
fn the_otg_hs_block_is_at_0x4204_0000_with_4_kib_of_fifo_and_nine_endpoints() {
    assert_eq!(OTG_HS_BASE, 0x4204_0000);
    assert_eq!(OTG_FIFO_WORDS, 1024);
    assert_eq!(OTG_ENDPOINTS, 9);
}
