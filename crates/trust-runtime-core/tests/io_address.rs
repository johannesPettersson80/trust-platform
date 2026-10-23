use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

#[test]
fn direct_addresses_preserve_area_width_bit_and_hierarchical_components() {
    let bit = IoAddress::parse("%IX2.7").unwrap();
    assert_eq!(bit.area, IoArea::Input);
    assert_eq!(bit.size, IoSize::Bit);
    assert_eq!(bit.byte, 2);
    assert_eq!(bit.bit, 7);
    assert_eq!(bit.path, vec![2]);
    let hierarchical = IoAddress::parse("%QW3.4.5").unwrap();
    assert_eq!(hierarchical.area, IoArea::Output);
    assert_eq!(hierarchical.size, IoSize::Word);
    assert_eq!(hierarchical.path, vec![3, 4, 5]);
    assert!(IoAddress::parse("%M*").unwrap().wildcard);
}

#[test]
fn malformed_direct_addresses_reject_without_panicking() {
    for text in [
        "",
        "%",
        "%I",
        "%IX",
        "%IX0.8",
        "%QW4294967296",
        "%IX.0",
        "%QB-1",
        "%MW1..2",
        "%AB0",
    ] {
        assert!(IoAddress::parse(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn flat_extents_preserve_width_and_separate_hierarchical_storage() {
    use trust_runtime_core::io_address::IoAddress;
    assert_eq!(
        IoAddress::parse("%MW4096")
            .unwrap()
            .flat_byte_end()
            .unwrap(),
        Some(4098)
    );
    assert_eq!(
        IoAddress::parse("%IX7.3").unwrap().flat_byte_end().unwrap(),
        Some(8)
    );
    assert_eq!(
        IoAddress::parse("%MW1.2").unwrap().flat_byte_end().unwrap(),
        None
    );
    assert!(IoAddress::parse("%ML4294967295")
        .unwrap()
        .flat_byte_end()
        .is_err());
    assert!(IoAddress::parse("%M*").unwrap().flat_byte_end().is_err());
}
