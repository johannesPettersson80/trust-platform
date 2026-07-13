use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_void,
    mem::{align_of, offset_of, size_of},
    time::Duration,
};

use crate::{
    ffi::{test_owner, FunctionTable},
    AdsDeviceState, AdsError, AmsAddress, AmsNetId, TcAdsDll,
};

#[derive(Debug, Default)]
struct FakeState {
    closes: Vec<i32>,
    close_results: VecDeque<i32>,
    timeout: Option<(i32, i32)>,
    last_target: Option<AmsAddress>,
    last_index: Option<(u32, u32)>,
    written: Vec<u8>,
    next_error: i32,
    invalid_read_length: bool,
}

thread_local! {
    static FAKE: RefCell<FakeState> = RefCell::new(FakeState::default());
}

unsafe extern "system" fn fake_open() -> i32 {
    42
}

unsafe extern "system" fn fake_open_failure() -> i32 {
    0
}

unsafe extern "system" fn fake_close(port: i32) -> i32 {
    FAKE.with(|state| {
        let mut state = state.borrow_mut();
        state.closes.push(port);
        state.close_results.pop_front().unwrap_or(0)
    })
}

unsafe extern "system" fn fake_local_address(port: i32, address: *mut AmsAddress) -> i32 {
    if port != 42 {
        return 1;
    }
    // SAFETY: FunctionTable guarantees a live, aligned output address.
    unsafe {
        address.write(AmsAddress::new(
            AmsNetId::new([10, 20, 30, 40, 1, 1]),
            32905,
        ));
    };
    0
}

unsafe extern "system" fn fake_timeout(port: i32, millis: i32) -> i32 {
    FAKE.with(|state| {
        let mut state = state.borrow_mut();
        state.timeout = Some((port, millis));
        state.next_error
    })
}

unsafe extern "system" fn fake_read_state(
    _port: i32,
    target: *const AmsAddress,
    ads_state: *mut u16,
    device_state: *mut u16,
) -> i32 {
    // SAFETY: FunctionTable guarantees live input and output pointers.
    let target = unsafe { *target };
    FAKE.with(|state| state.borrow_mut().last_target = Some(target));
    // SAFETY: FunctionTable owns both exclusive u16 outputs for this call.
    unsafe {
        ads_state.write(5);
        device_state.write(9);
    }
    0
}

unsafe extern "system" fn fake_read(
    _port: i32,
    target: *const AmsAddress,
    index_group: u32,
    index_offset: u32,
    length: u32,
    data: *mut c_void,
    returned: *mut u32,
) -> i32 {
    // SAFETY: FunctionTable guarantees a live target for the synchronous call.
    let target = unsafe { *target };
    let (error, invalid) = FAKE.with(|state| {
        let mut state = state.borrow_mut();
        state.last_target = Some(target);
        state.last_index = Some((index_group, index_offset));
        (state.next_error, state.invalid_read_length)
    });
    if error != 0 {
        return error;
    }
    let length_usize = usize::try_from(length).unwrap_or(0);
    if length_usize > 0 {
        // SAFETY: FunctionTable supplies exactly `length` writable bytes.
        let bytes = unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), length_usize) };
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::try_from(index + 1).unwrap_or(u8::MAX);
        }
    }
    // SAFETY: FunctionTable supplies a live u32 output.
    unsafe {
        returned.write(if invalid {
            length.saturating_add(1)
        } else {
            length
        });
    };
    0
}

unsafe extern "system" fn fake_write(
    _port: i32,
    target: *const AmsAddress,
    index_group: u32,
    index_offset: u32,
    length: u32,
    data: *const c_void,
) -> i32 {
    // SAFETY: FunctionTable guarantees a live target for the synchronous call.
    let target = unsafe { *target };
    let length = usize::try_from(length).unwrap_or(0);
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: FunctionTable supplies exactly `length` immutable bytes.
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) }
    };
    FAKE.with(|state| {
        let mut state = state.borrow_mut();
        state.last_target = Some(target);
        state.last_index = Some((index_group, index_offset));
        state.written = bytes.to_vec();
        state.next_error
    })
}

#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn fake_read_write(
    port: i32,
    target: *const AmsAddress,
    index_group: u32,
    index_offset: u32,
    read_length: u32,
    read_data: *mut c_void,
    write_length: u32,
    write_data: *const c_void,
    returned: *mut u32,
) -> i32 {
    // SAFETY: the fake forwards the same validated pointers and lengths to its
    // read/write halves without retaining them.
    let write_code = unsafe {
        fake_write(
            port,
            target,
            index_group,
            index_offset,
            write_length,
            write_data,
        )
    };
    if write_code != 0 {
        return write_code;
    }
    // SAFETY: same invariant as above for the read half.
    unsafe {
        fake_read(
            port,
            target,
            index_group,
            index_offset,
            read_length,
            read_data,
            returned,
        )
    }
}

fn fake_table() -> FunctionTable {
    FunctionTable {
        port_open: fake_open,
        port_close: fake_close,
        get_local_address: fake_local_address,
        set_timeout: fake_timeout,
        read_state: fake_read_state,
        read: fake_read,
        write: fake_write,
        read_write: fake_read_write,
    }
}

fn reset() {
    FAKE.with(|state| *state.borrow_mut() = FakeState::default());
}

#[test]
fn beckhoff_address_abi_layout_is_exact() {
    assert_eq!(size_of::<AmsNetId>(), 6);
    assert_eq!(align_of::<AmsNetId>(), 1);
    assert_eq!(size_of::<AmsAddress>(), 8);
    assert_eq!(align_of::<AmsAddress>(), 2);
    assert_eq!(offset_of!(AmsAddress, net_id), 0);
    assert_eq!(offset_of!(AmsAddress, port), 6);
    assert_eq!(size_of::<i32>(), 4, "Windows LONG ABI");
    assert_eq!(size_of::<u32>(), 4, "Windows ULONG ABI");
    assert_eq!(size_of::<u16>(), 2, "Windows USHORT ABI");
}

#[test]
fn net_id_text_requires_exactly_six_bytes() {
    let parsed: AmsNetId = "10.20.30.40.1.1".parse().expect("valid AMS Net ID");
    assert_eq!(parsed.octets, [10, 20, 30, 40, 1, 1]);
    assert_eq!(parsed.to_string(), "10.20.30.40.1.1");
    assert!("10.20.30.40.1".parse::<AmsNetId>().is_err());
    assert!("10.20.30.40.1.1.2".parse::<AmsNetId>().is_err());
    assert!("100.67.999.217.1.1".parse::<AmsNetId>().is_err());
}

#[test]
fn fake_function_table_proves_safe_raii_and_byte_operations_on_linux() {
    reset();
    let library = TcAdsDll::from_owner(test_owner(fake_table()));
    let mut port = library.open_port().expect("fake router port");
    assert_eq!(port.port_number(), 42);
    assert_eq!(
        port.local_address().expect("local router identity"),
        AmsAddress::new(AmsNetId::new([10, 20, 30, 40, 1, 1]), 32905)
    );

    port.set_timeout(Duration::from_millis(2_500))
        .expect("set timeout");
    let target = AmsAddress::new(AmsNetId::new([10, 20, 30, 40, 1, 1]), 851);
    assert_eq!(
        port.read_state(&target).expect("read state"),
        AdsDeviceState {
            ads_state: 5,
            device_state: 9,
        }
    );

    let mut read = [0_u8; 4];
    assert_eq!(port.read(&target, 0xF005, 12, &mut read), Ok(4));
    assert_eq!(read, [1, 2, 3, 4]);
    port.write(&target, 0xF005, 13, &[9, 8, 7])
        .expect("write bytes");

    let mut response = [0_u8; 3];
    assert_eq!(
        port.read_write(&target, 0xF003, 14, &mut response, b"MAIN.value\0"),
        Ok(3)
    );
    assert_eq!(response, [1, 2, 3]);

    FAKE.with(|state| {
        let state = state.borrow();
        assert_eq!(state.timeout, Some((42, 2_500)));
        assert_eq!(state.last_target, Some(target));
        assert_eq!(state.last_index, Some((0xF003, 14)));
        assert_eq!(state.written, b"MAIN.value\0");
        assert!(state.closes.is_empty());
    });

    drop(port);
    FAKE.with(|state| assert_eq!(state.borrow().closes, vec![42]));
}

#[test]
fn explicit_close_does_not_close_twice_on_drop() {
    reset();
    let library = TcAdsDll::from_owner(test_owner(fake_table()));
    library
        .open_port()
        .expect("fake router port")
        .close()
        .expect("native close");
    FAKE.with(|state| assert_eq!(state.borrow().closes, vec![42]));
}

#[test]
fn failed_explicit_close_retains_ownership_for_one_drop_retry() {
    reset();
    FAKE.with(|state| {
        state.borrow_mut().close_results = VecDeque::from([0x745, 0]);
    });
    let library = TcAdsDll::from_owner(test_owner(fake_table()));

    let error = library
        .open_port()
        .expect("fake router port")
        .close()
        .expect_err("first native close must fail");

    let AdsError::Call {
        operation,
        code,
        description,
    } = error
    else {
        panic!("expected typed native close error");
    };
    assert_eq!(operation, "AdsPortCloseEx");
    assert_eq!(code, 0x745);
    assert!(description.to_ascii_lowercase().contains("timed out"));
    FAKE.with(|state| {
        assert_eq!(
            state.borrow().closes,
            vec![42, 42],
            "the failed explicit close must leave the port owned so Drop retries once"
        );
    });
}

#[test]
fn errors_are_typed_before_they_cross_the_safe_boundary() {
    reset();
    let mut table = fake_table();
    table.port_open = fake_open_failure;
    let library = TcAdsDll::from_owner(test_owner(table));
    assert!(matches!(library.open_port(), Err(AdsError::PortOpenFailed)));

    let library = TcAdsDll::from_owner(test_owner(fake_table()));
    let mut port = library.open_port().expect("fake router port");
    FAKE.with(|state| state.borrow_mut().next_error = 0x701);
    assert_eq!(
        port.set_timeout(Duration::from_secs(1)),
        Err(AdsError::Call {
            operation: "AdsSyncSetTimeoutEx",
            code: 0x701,
            description: "ADS service is not supported by the target",
        })
    );

    FAKE.with(|state| {
        let mut state = state.borrow_mut();
        state.next_error = 0;
        state.invalid_read_length = true;
    });
    let target = AmsAddress::new(AmsNetId::new([1, 2, 3, 4, 5, 6]), 851);
    assert_eq!(
        port.read(&target, 1, 2, &mut [0; 2]),
        Err(AdsError::InvalidReadLength {
            operation: "AdsSyncReadReqEx2",
            reported: 3,
            capacity: 2,
        })
    );
    port.set_timeout(Duration::from_millis(i32::MAX as u64))
        .expect("largest signed LONG timeout");
    FAKE.with(|state| assert_eq!(state.borrow().timeout, Some((42, i32::MAX))));
    assert!(matches!(
        port.set_timeout(Duration::from_millis(i32::MAX as u64 + 1)),
        Err(AdsError::TimeoutTooLarge { .. })
    ));
}

#[cfg(not(windows))]
#[test]
fn installed_loader_is_explicitly_windows_only() {
    assert!(matches!(
        TcAdsDll::load_installed(),
        Err(AdsError::UnsupportedPlatform)
    ));
    assert!(matches!(
        crate::trusted_program_data_root(),
        Err(AdsError::UnsupportedPlatform)
    ));
}
