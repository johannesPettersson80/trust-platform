use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoSize};
use trust_runtime::memory::IoArea;

#[test]
fn parse_addresses() {
    let addr = IoAddress::parse("%IX0.3").unwrap();
    assert_eq!(addr.area, IoArea::Input);
    assert_eq!(addr.size, IoSize::Bit);
    assert_eq!(addr.byte, 0);
    assert_eq!(addr.bit, 3);

    let addr = IoAddress::parse("%QW2").unwrap();
    assert_eq!(addr.area, IoArea::Output);
    assert_eq!(addr.size, IoSize::Word);
    assert_eq!(addr.byte, 2);
    assert_eq!(addr.bit, 0);

    let addr = IoAddress::parse("%MB5").unwrap();
    assert_eq!(addr.area, IoArea::Memory);
    assert_eq!(addr.size, IoSize::Byte);
    assert_eq!(addr.byte, 5);

    let addr = IoAddress::parse("%MX0.7").unwrap();
    assert_eq!(addr.area, IoArea::Memory);
    assert_eq!(addr.size, IoSize::Bit);
    assert_eq!(addr.byte, 0);
    assert_eq!(addr.bit, 7);

    let addr = IoAddress::parse("%MW12").unwrap();
    assert_eq!(addr.area, IoArea::Memory);
    assert_eq!(addr.size, IoSize::Word);
    assert_eq!(addr.byte, 12);

    let addr = IoAddress::parse("%MD24").unwrap();
    assert_eq!(addr.area, IoArea::Memory);
    assert_eq!(addr.size, IoSize::DWord);
    assert_eq!(addr.byte, 24);

    let addr = IoAddress::parse("%ML40").unwrap();
    assert_eq!(addr.area, IoArea::Memory);
    assert_eq!(addr.size, IoSize::LWord);
    assert_eq!(addr.byte, 40);
}

#[test]
fn direct_address_parser_covers_hierarchy_and_exact_wildcards() {
    let hierarchy = IoAddress::parse("%IX1.2.3").expect("hierarchical bit address");
    assert_eq!(hierarchy.area, IoArea::Input);
    assert_eq!(hierarchy.size, IoSize::Bit);
    assert_eq!(hierarchy.byte, 1);
    assert_eq!(hierarchy.bit, 3);
    assert_eq!(hierarchy.path, vec![1, 2]);
    assert!(!hierarchy.wildcard);

    let implicit_size = IoAddress::parse("%Q7").expect("unsized bit address");
    assert_eq!(implicit_size.area, IoArea::Output);
    assert_eq!(implicit_size.size, IoSize::Bit);
    assert_eq!(implicit_size.byte, 7);
    assert_eq!(implicit_size.bit, 0);
    assert_eq!(implicit_size.path, vec![7]);
    assert!(!implicit_size.wildcard);

    let word_hierarchy = IoAddress::parse("%IW2.5.7.1").expect("hierarchical word address");
    assert_eq!(word_hierarchy.area, IoArea::Input);
    assert_eq!(word_hierarchy.size, IoSize::Word);
    assert_eq!(word_hierarchy.byte, 2);
    assert_eq!(word_hierarchy.bit, 0);
    assert_eq!(word_hierarchy.path, vec![2, 5, 7, 1]);
    assert!(!word_hierarchy.wildcard);

    for (text, area) in [
        ("%I*", IoArea::Input),
        ("%Q*", IoArea::Output),
        ("%M*", IoArea::Memory),
    ] {
        let wildcard = IoAddress::parse(text).expect("exact wildcard address");
        assert_eq!(wildcard.area, area);
        assert_eq!(wildcard.size, IoSize::Bit);
        assert_eq!(wildcard.byte, 0);
        assert_eq!(wildcard.bit, 0);
        assert!(wildcard.path.is_empty());
        assert!(wildcard.wildcard);
    }

    assert!(
        IoAddress::parse(" \t%Q* \n")
            .expect("surrounding whitespace is ignored")
            .wildcard
    );

    let largest_component = IoAddress::parse("%MW4294967295").expect("largest u32 component");
    assert_eq!(largest_component.byte, u32::MAX);
    assert_eq!(largest_component.path, vec![u32::MAX]);
}

#[test]
fn direct_address_parser_rejects_malformed_wildcards() {
    for text in [
        "%I*garbage",
        "%IX*",
        "%IB*",
        "%IX *",
        "%I *",
        "%Q**",
        "%M*.1",
    ] {
        assert_invalid_address(text);
    }
}

#[test]
fn direct_address_parser_rejects_non_decimal_and_out_of_range_components() {
    for text in [
        "%I",
        "%Z0",
        "%IX",
        "%IX.",
        "%IX1.",
        "%IX.1",
        "%IW+1",
        "%IW-1",
        "%IW１２",
        "%IW١",
        "%IW4294967296",
        "%IX1.8",
        "%IX1.256",
        "%IX1..2",
        "%iw1",
        "%IZ1",
    ] {
        assert_invalid_address(text);
    }
}

fn assert_invalid_address(text: &str) {
    match IoAddress::parse(text) {
        Err(RuntimeError::InvalidIoAddress(value)) => {
            assert_eq!(value.as_str(), text.trim(), "{text}")
        }
        other => panic!("{text} must be rejected as InvalidIoAddress, got {other:?}"),
    }
}

#[test]
fn bit_and_word_access() {
    let mut io = trust_runtime::io::IoInterface::new();
    let bit = IoAddress::parse("%IX1.2").unwrap();
    let word = IoAddress::parse("%QW0").unwrap();

    io.write(&bit, trust_runtime::value::Value::Bool(true))
        .unwrap();
    let value = io.read(&bit).unwrap();
    assert_eq!(value, trust_runtime::value::Value::Bool(true));

    io.write(&word, trust_runtime::value::Value::Word(0x1234))
        .unwrap();
    let value = io.read(&word).unwrap();
    assert_eq!(value, trust_runtime::value::Value::Word(0x1234));
}
