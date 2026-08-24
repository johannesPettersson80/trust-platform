use super::*;

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("I/O address")
}

fn bytes_address(text: &str, len: u32) -> IoAddress {
    let mut address = address(text);
    address.size = IoSize::Bytes(len);
    address
}

#[test]
fn new_interface_has_three_empty_areas_and_no_bindings() {
    let interface = IoInterface::new();

    assert!(interface.inputs().is_empty());
    assert!(interface.outputs().is_empty());
    assert!(interface.memory().is_empty());
    assert!(interface.bindings().is_empty());
    assert!(interface.hierarchical.is_empty());
}

#[test]
fn resize_preserves_prefix_truncates_suffix_and_zero_fills_growth() {
    let mut interface = IoInterface::new();
    interface.try_resize(3, 3, 3).unwrap();
    interface.inputs_mut().copy_from_slice(&[1, 2, 3]);
    interface.outputs_mut().copy_from_slice(&[4, 5, 6]);
    interface.memory_mut().copy_from_slice(&[7, 8, 9]);

    interface.try_resize(2, 4, 3).unwrap();

    assert_eq!(interface.inputs(), &[1, 2]);
    assert_eq!(interface.outputs(), &[4, 5, 6, 0]);
    assert_eq!(interface.memory(), &[7, 8, 9]);
}

#[test]
fn failed_resize_is_atomic_across_all_three_areas() {
    let mut interface = IoInterface::new();
    interface.try_resize(2, 2, 2).unwrap();
    interface.inputs_mut().copy_from_slice(&[1, 2]);
    interface.outputs_mut().copy_from_slice(&[3, 4]);
    interface.memory_mut().copy_from_slice(&[5, 6]);

    let error = interface
        .try_resize(4, 5, PROCESS_IMAGE_AREA_LIMIT + 1)
        .expect_err("oversized memory area");

    assert!(error.to_string().contains("area limit"));
    assert_eq!(interface.inputs(), &[1, 2]);
    assert_eq!(interface.outputs(), &[3, 4]);
    assert_eq!(interface.memory(), &[5, 6]);
}

#[test]
fn bit_write_changes_only_selected_bit() {
    let mut interface = IoInterface::new();
    interface.try_resize(0, 0, 1).unwrap();
    interface.memory_mut()[0] = 0b1010_0101;
    let bit = address("%MX0.1");

    interface.write(&bit, Value::Bool(true)).unwrap();
    assert_eq!(interface.memory()[0], 0b1010_0111);
    assert_eq!(interface.read(&bit).unwrap(), Value::Bool(true));

    interface.write(&bit, Value::Bool(false)).unwrap();
    assert_eq!(interface.memory()[0], 0b1010_0101);
    assert_eq!(interface.read(&bit).unwrap(), Value::Bool(false));
}

#[test]
fn byte_word_dword_and_lword_round_trip_little_endian() {
    let cases = [
        ("%MB0", Value::Byte(0xA5), vec![0xA5]),
        ("%MW1", Value::Word(0x1234), vec![0x34, 0x12]),
        (
            "%MD3",
            Value::DWord(0x1234_5678),
            vec![0x78, 0x56, 0x34, 0x12],
        ),
        (
            "%ML7",
            Value::LWord(0x0123_4567_89AB_CDEF),
            vec![0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01],
        ),
    ];

    for (text, value, expected_bytes) in cases {
        let mut interface = IoInterface::new();
        let address = address(text);
        interface.write(&address, value.clone()).unwrap();
        let start = address.byte as usize;
        assert_eq!(
            &interface.memory()[start..start + expected_bytes.len()],
            expected_bytes
        );
        assert_eq!(interface.read(&address).unwrap(), value);
    }
}

#[test]
fn write_targets_only_the_selected_process_image_area() {
    let mut interface = IoInterface::new();
    interface.try_resize(1, 1, 1).unwrap();
    interface.inputs_mut()[0] = 1;
    interface.outputs_mut()[0] = 2;
    interface.memory_mut()[0] = 3;

    interface
        .write(&address("%QB0"), Value::Byte(9))
        .unwrap();

    assert_eq!(interface.inputs(), &[1]);
    assert_eq!(interface.outputs(), &[9]);
    assert_eq!(interface.memory(), &[3]);
}

#[test]
fn wrong_value_kind_rejects_without_allocation_or_mutation() {
    let cases = [
        ("%MX0.0", Value::Byte(1)),
        ("%MB0", Value::Word(1)),
        ("%MW0", Value::DWord(1)),
        ("%MD0", Value::LWord(1)),
        ("%ML0", Value::Bool(true)),
    ];

    for (text, value) in cases {
        let mut interface = IoInterface::new();
        let error = interface
            .write(&address(text), value)
            .expect_err("wrong value kind");
        assert_eq!(error, RuntimeError::TypeMismatch);
        assert!(interface.memory().is_empty());
    }
}

#[test]
fn wildcard_read_and_write_fail_without_mutation() {
    let mut interface = IoInterface::new();
    let wildcard = address("%M*");

    assert!(matches!(
        interface.read(&wildcard),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert!(matches!(
        interface.write(&wildcard, Value::Byte(1)),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert!(interface.memory().is_empty());
    assert!(interface.hierarchical.is_empty());
}

#[test]
fn zero_length_string_window_is_rejected_without_allocation() {
    let mut interface = IoInterface::new();
    let address = bytes_address("%MB0", 0);

    assert!(matches!(
        validate_process_image_address(&address),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert!(matches!(
        interface.write(&address, Value::String(SmolStr::new(""))),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert!(interface.memory().is_empty());
}

#[test]
fn fixed_string_write_zero_fills_remainder_and_erases_old_suffix() {
    let mut interface = IoInterface::new();
    let address = bytes_address("%QB1", 6);

    interface
        .write(&address, Value::String(SmolStr::new("ABCDE")))
        .unwrap();
    interface
        .write(&address, Value::String(SmolStr::new("XY")))
        .unwrap();

    assert_eq!(&interface.outputs()[1..7], b"XY\0\0\0\0");
    assert_eq!(
        interface.read(&address).unwrap(),
        Value::String(SmolStr::new("XY"))
    );
}

#[test]
fn fixed_string_window_counts_utf8_bytes_not_characters() {
    let mut interface = IoInterface::new();
    let fits = bytes_address("%MB0", 4);
    let too_short = bytes_address("%MB8", 3);

    interface
        .write(&fits, Value::String(SmolStr::new("åä")))
        .unwrap();
    let error = interface
        .write(&too_short, Value::String(SmolStr::new("åä")))
        .expect_err("four UTF-8 bytes do not fit three-byte window");

    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(
        interface.read(&fits).unwrap(),
        Value::String(SmolStr::new("åä"))
    );
    assert_eq!(interface.memory().len(), 4);
}

#[test]
fn fixed_string_read_stops_at_nul_and_rejects_invalid_utf8_prefix() {
    let mut interface = IoInterface::new();
    interface.try_resize(4, 0, 4).unwrap();
    interface.inputs_mut().copy_from_slice(b"OK\0X");
    interface.memory_mut().copy_from_slice(&[0xFF, 0, 0, 0]);

    assert_eq!(
        interface.read(&bytes_address("%IB0", 4)).unwrap(),
        Value::String(SmolStr::new("OK"))
    );
    assert_eq!(
        interface
            .read(&bytes_address("%MB0", 4))
            .expect_err("invalid UTF-8"),
        RuntimeError::TypeMismatch
    );
}

#[test]
fn unallocated_multibyte_reads_zero_fill_without_growing() {
    let interface = IoInterface::new();

    assert_eq!(
        interface.read(&address("%IW10")).unwrap(),
        Value::Word(0)
    );
    assert_eq!(
        interface.read(&address("%QD20")).unwrap(),
        Value::DWord(0)
    );
    assert_eq!(
        interface.read(&address("%ML30")).unwrap(),
        Value::LWord(0)
    );
    assert!(interface.inputs().is_empty());
    assert!(interface.outputs().is_empty());
    assert!(interface.memory().is_empty());
}

#[test]
fn concrete_address_validation_checks_complete_width_at_cap() {
    for (size, width) in [
        (IoSize::Bit, 1usize),
        (IoSize::Byte, 1),
        (IoSize::Word, 2),
        (IoSize::DWord, 4),
        (IoSize::LWord, 8),
        (IoSize::Bytes(13), 13),
    ] {
        let mut valid = address("%MB0");
        valid.size = size;
        valid.byte = (PROCESS_IMAGE_AREA_LIMIT - width) as u32;
        validate_process_image_address(&valid).unwrap();

        let mut invalid = valid.clone();
        invalid.byte += 1;
        assert!(matches!(
            validate_process_image_address(&invalid),
            Err(RuntimeError::InvalidIoAddress(_))
        ));
    }
}

#[test]
fn hierarchical_values_are_exact_and_do_not_allocate_flat_images() {
    let mut interface = IoInterface::new();
    let mut first = address("%MB0");
    first.path = vec![1, 2];
    let mut second = first.clone();
    second.path = vec![1, 3];

    interface.write(&first, Value::DInt(11)).unwrap();
    interface.write(&second, Value::DInt(22)).unwrap();

    assert_eq!(interface.read(&first).unwrap(), Value::DInt(11));
    assert_eq!(interface.read(&second).unwrap(), Value::DInt(22));
    assert!(interface.inputs().is_empty());
    assert!(interface.outputs().is_empty());
    assert!(interface.memory().is_empty());
}

#[test]
fn absent_hierarchical_value_fails_instead_of_fabricating_zero() {
    let interface = IoInterface::new();
    let mut address = address("%MB0");
    address.path = vec![7, 9];

    assert!(matches!(
        interface.read(&address),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
}

#[test]
fn hierarchical_key_includes_area_size_bit_and_complete_path() {
    let mut interface = IoInterface::new();
    let mut base = address("%MX0.1");
    base.path = vec![4, 5];
    let mut other_area = base.clone();
    other_area.area = IoArea::Output;
    let mut other_size = base.clone();
    other_size.size = IoSize::Byte;
    let mut other_bit = base.clone();
    other_bit.bit = 2;
    let mut other_path = base.clone();
    other_path.path.push(6);

    for (address, value) in [
        (&base, Value::DInt(1)),
        (&other_area, Value::DInt(2)),
        (&other_size, Value::DInt(3)),
        (&other_bit, Value::DInt(4)),
        (&other_path, Value::DInt(5)),
    ] {
        interface.write(address, value).unwrap();
    }

    assert_eq!(interface.read(&base).unwrap(), Value::DInt(1));
    assert_eq!(interface.read(&other_area).unwrap(), Value::DInt(2));
    assert_eq!(interface.read(&other_size).unwrap(), Value::DInt(3));
    assert_eq!(interface.read(&other_bit).unwrap(), Value::DInt(4));
    assert_eq!(interface.read(&other_path).unwrap(), Value::DInt(5));
}

#[test]
fn binding_variants_preserve_target_type_name_and_order() {
    let mut interface = IoInterface::new();
    interface.bind("Input", address("%IX0.0"));
    interface.bind_typed("Memory", address("%MW0"), TypeId::INT);

    assert_eq!(interface.bindings().len(), 2);
    assert!(matches!(
        &interface.bindings()[0].target,
        IoTarget::Name(name) if name == "Input"
    ));
    assert_eq!(
        interface.bindings()[0].display_name.as_deref(),
        Some("Input")
    );
    assert_eq!(interface.bindings()[0].value_type, None);
    assert!(matches!(
        &interface.bindings()[1].target,
        IoTarget::Name(name) if name == "Memory"
    ));
    assert_eq!(
        interface.bindings()[1].display_name.as_deref(),
        Some("Memory")
    );
    assert_eq!(interface.bindings()[1].value_type, Some(TypeId::INT));
}

#[test]
fn binding_source_refresh_replaces_and_clears_prior_values() {
    let mut interface = IoInterface::new();
    interface.bind("Input", address("%IX0.0"));
    interface.bind("Output", address("%QX0.0"));
    interface.set_binding_sources(|_| Some(SmolStr::new("first")));
    assert!(interface
        .bindings()
        .iter()
        .all(|binding| binding.source.as_deref() == Some("first")));

    interface.set_binding_sources(|address| {
        matches!(address.area, IoArea::Output).then(|| SmolStr::new("second"))
    });

    assert_eq!(interface.bindings()[0].source, None);
    assert_eq!(interface.bindings()[1].source.as_deref(), Some("second"));
}

#[test]
fn snapshot_partitions_areas_and_preserves_order_within_each_area() {
    let mut interface = IoInterface::new();
    interface.bind("Q1", address("%QX0.1"));
    interface.bind("I1", address("%IX0.1"));
    interface.bind("M1", address("%MX0.1"));
    interface.bind("I2", address("%IX0.2"));
    interface.bind("Q2", address("%QX0.2"));

    let snapshot = interface.snapshot();
    fn names(entries: &[IoSnapshotEntry]) -> Vec<&str> {
        entries
            .iter()
            .map(|entry| entry.name.as_deref().unwrap())
            .collect()
    }

    assert_eq!(names(&snapshot.inputs), vec!["I1", "I2"]);
    assert_eq!(names(&snapshot.outputs), vec!["Q1", "Q2"]);
    assert_eq!(names(&snapshot.memory), vec!["M1"]);
}

#[test]
fn wildcard_snapshot_is_unresolved_without_concrete_read() {
    let mut interface = IoInterface::new();
    interface.bind("AnyMemory", address("%M*"));

    let snapshot = interface.snapshot();

    assert_eq!(snapshot.memory.len(), 1);
    assert!(matches!(
        &snapshot.memory[0].value,
        IoSnapshotValue::Unresolved
    ));
}

#[test]
fn read_inputs_loads_input_and_memory_but_ignores_output_bindings() {
    let mut interface = IoInterface::new();
    interface.bind("Input", address("%IB0"));
    interface.bind("Output", address("%QB0"));
    interface.bind("Memory", address("%MB0"));
    interface.write(&address("%IB0"), Value::Byte(1)).unwrap();
    interface.write(&address("%QB0"), Value::Byte(2)).unwrap();
    interface.write(&address("%MB0"), Value::Byte(3)).unwrap();
    let mut storage = VariableStorage::new();
    storage.set_global("Input", Value::Byte(9));
    storage.set_global("Output", Value::Byte(9));
    storage.set_global("Memory", Value::Byte(9));

    interface.read_inputs(&mut storage).unwrap();

    assert_eq!(storage.get_global("Input"), Some(&Value::Byte(1)));
    assert_eq!(storage.get_global("Memory"), Some(&Value::Byte(3)));
    assert_eq!(storage.get_global("Output"), Some(&Value::Byte(9)));
}

#[test]
fn read_inputs_is_atomic_when_a_later_binding_fails() {
    let mut interface = IoInterface::new();
    interface.try_resize(3, 0, 0).unwrap();
    interface.inputs_mut().copy_from_slice(&[1, 0xFF, 0]);
    interface.bind("First", address("%IB0"));
    interface.bind("InvalidUtf8", bytes_address("%IB1", 2));
    let mut storage = VariableStorage::new();
    storage.set_global("First", Value::Byte(9));
    storage.set_global("InvalidUtf8", Value::String(SmolStr::new("old")));

    let error = interface
        .read_inputs(&mut storage)
        .expect_err("later invalid UTF-8 binding");

    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(storage.get_global("First"), Some(&Value::Byte(9)));
    assert_eq!(
        storage.get_global("InvalidUtf8"),
        Some(&Value::String(SmolStr::new("old")))
    );
}

#[test]
fn write_outputs_commits_output_and_memory_but_ignores_input_bindings() {
    let mut interface = IoInterface::new();
    interface.bind("Input", address("%IB0"));
    interface.bind("Output", address("%QB0"));
    interface.bind("Memory", address("%MB0"));
    interface.try_resize(1, 1, 1).unwrap();
    interface.inputs_mut()[0] = 7;
    let mut storage = VariableStorage::new();
    storage.set_global("Input", Value::Byte(1));
    storage.set_global("Output", Value::Byte(2));
    storage.set_global("Memory", Value::Byte(3));

    interface.write_outputs(&storage).unwrap();

    assert_eq!(interface.inputs(), &[7]);
    assert_eq!(interface.outputs(), &[2]);
    assert_eq!(interface.memory(), &[3]);
}

#[test]
fn write_outputs_is_atomic_when_a_later_binding_fails() {
    let mut interface = IoInterface::new();
    interface.try_resize(0, 8, 8).unwrap();
    interface.outputs_mut().fill(0xA5);
    interface.memory_mut().fill(0x5A);
    interface.bind("Valid", address("%QB0"));
    interface.bind("TooLong", bytes_address("%MB0", 3));
    let mut storage = VariableStorage::new();
    storage.set_global("Valid", Value::Byte(1));
    storage.set_global("TooLong", Value::String(SmolStr::new("four")));

    let error = interface
        .write_outputs(&storage)
        .expect_err("later string overflow");

    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(interface.outputs(), &[0xA5; 8]);
    assert_eq!(interface.memory(), &[0x5A; 8]);
}
