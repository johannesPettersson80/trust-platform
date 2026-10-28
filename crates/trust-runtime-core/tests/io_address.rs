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

/// Specification 10 §9.2: an address is written in the form the parser reads, so every reply and
/// refusal that names an address can be read back. Each expected text is worked out from the rule:
/// the size prefix written out (`X` for a bit), every location component, the bit index after a
/// dot, a wildcard as the area and the asterisk only, a `STRING` byte window as its byte address.
/// Catches a sized wildcard (`%QX*`, which the parser refuses), a hierarchical address cut to its
/// first component (`%QW3` for `%QW3.4.5`), and a byte window written with another width (`%QL24`).
#[test]
fn an_address_is_written_as_the_parser_reads_it() {
    for (text, written) in [
        ("%IX0.1", "%IX0.1"),
        ("%I0.1", "%IX0.1"),
        ("%IX5", "%IX5.0"),
        ("%IX1.2.3", "%IX1.2.3"),
        ("%QW4", "%QW4"),
        ("%QW3.4.5", "%QW3.4.5"),
        ("%IB7", "%IB7"),
        ("%MD2", "%MD2"),
        ("%QL8", "%QL8"),
        (" %Q* ", "%Q*"),
        ("%I*", "%I*"),
        ("%M*", "%M*"),
    ] {
        let address = IoAddress::parse(text).unwrap();
        assert_eq!(address.to_string(), written, "{text:?}");
        assert_eq!(IoAddress::parse(written).unwrap(), address, "{text:?}");
    }
    let mut window = IoAddress::parse("%QB24").unwrap();
    window.size = IoSize::Bytes(16);
    assert_eq!(window.to_string(), "%QB24");
}
