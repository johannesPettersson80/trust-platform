//! The tests of the `can` driver (specification 11 §6.5): its exchange over a fake controller,
//! its interface checks over a fake sysfs tree, and its frames and filters, none of which needs
//! a CAN interface.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use smol_str::SmolStr;
use socketcan::{EmbeddedFrame, Frame};
use trust_runtime_services::io_drivers::can::{
    CanFrame, CanId, CanParams, CanReceive, CanTransmit,
};
use trust_runtime_services::io_drivers::OnError;

use super::{
    bus_off_change, check_bitrate, check_interface, check_pins, data_frame, filter_words,
    socket_frame, CanDriver, CanPort, Received,
};
use crate::io::fake_sysfs::FakeSysfs;
use crate::io::{IoAddress, IoDriver, IoDriverHealth};

/// The receive's frame.
const RX: CanId = CanId {
    id: 0x181,
    extended: false,
};

/// The transmit's frame.
const TX: CanId = CanId {
    id: 0x201,
    extended: false,
};

/// What a fake controller holds: what it received and has not given yet, what it sent, the
/// failure of a send, and the bus-off state it reports.
#[derive(Default)]
struct Bus {
    received: VecDeque<Received>,
    sent: Vec<CanFrame>,
    send_failure: Option<String>,
    state: Option<bool>,
}

/// A controller over a `Bus` that the test holds as well.
struct FakePort(Arc<Mutex<Bus>>);

impl CanPort for FakePort {
    fn receive(&mut self) -> Result<Option<Received>, String> {
        Ok(self.0.lock().expect("bus").received.pop_front())
    }

    fn send(&mut self, frame: &CanFrame) -> Result<(), String> {
        let mut bus = self.0.lock().expect("bus");
        match bus.send_failure.clone() {
            Some(failure) => Err(failure),
            None => {
                bus.sent.push(*frame);
                Ok(())
            }
        }
    }

    fn bus_off(&mut self) -> Result<Option<bool>, String> {
        Ok(self.0.lock().expect("bus").state)
    }
}

/// `can0` at 500 kbit/s: the frame 0x181 into `%IB0` to `%IB3` with a timeout of 100 ms, and
/// the frame 0x201 from `%QB0` and `%QB1` every 100 ms.
fn params(on_error: OnError) -> CanParams {
    CanParams {
        line: SmolStr::new("can0"),
        tx: None,
        rx: None,
        bitrate: 500_000,
        receive: vec![CanReceive {
            id: RX,
            address: IoAddress::parse("%IB0").expect("receive address"),
            length: 4,
            timeout_ms: Some(100),
        }],
        transmit: vec![CanTransmit {
            id: TX,
            address: IoAddress::parse("%QB0").expect("transmit address"),
            length: 2,
            period_ms: 100,
        }],
        on_error,
    }
}

fn driver(on_error: OnError) -> (CanDriver, Arc<Mutex<Bus>>) {
    let bus = Arc::new(Mutex::new(Bus::default()));
    let driver = CanDriver::new(params(on_error), Box::new(FakePort(Arc::clone(&bus))));
    (driver, bus)
}

fn arrive(bus: &Mutex<Bus>, received: Received) {
    bus.lock().expect("bus").received.push_back(received);
}

fn frame(id: CanId, bytes: &[u8]) -> CanFrame {
    let mut data = [0; 8];
    data[..bytes.len()].copy_from_slice(bytes);
    CanFrame {
        id,
        len: u8::try_from(bytes.len()).expect("at most 8 bytes"),
        data,
    }
}

fn faulted(error: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: SmolStr::new(error),
    }
}

fn degraded(error: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: SmolStr::new(error),
    }
}

/// Specification 11 §6.5: a data frame with a receive's identifier writes its data bytes to
/// the receive's `length` bytes from `address`, and zeros where its data length code does not
/// reach. The frame 0x181 with the 2 bytes `12 34` fills `%IB0` to `%IB3` with `12 34 00 00`
/// and leaves `%IB4` and `%IB5`. Catches bytes of an older, longer frame left past the code,
/// and bytes written past `length`.
#[test]
fn a_received_frame_fills_its_bytes_and_zeros_past_its_length() {
    let (mut driver, bus) = driver(OnError::Fault);
    arrive(&bus, Received::Data(frame(RX, &[0x12, 0x34])));
    let mut inputs = [0xFF; 6];

    driver
        .exchange_inputs(&mut inputs, 10)
        .expect("read the frame");

    assert_eq!(inputs, [0x12, 0x34, 0x00, 0x00, 0xFF, 0xFF]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// Specification 11 §6.5: with `timeout_ms` 100, no frame 0x181 for that long faults the
/// driver's health with `no frame 0x181 for 100 ms`, and under `on_error = "fault"` the read
/// fails. A receive's timeout concerns received data alone, so the output path stays healthy
/// (specification 11, safe-state handoff confirmation). Catches a receive that never times
/// out, and a stale receive that leaves a safe-state handoff unconfirmed.
#[test]
fn a_receive_without_its_frame_for_its_timeout_faults_the_driver() {
    let (mut driver, _bus) = driver(OnError::Fault);
    let mut inputs = [0; 4];

    driver
        .exchange_inputs(&mut inputs, 50)
        .expect("50 ms is within the timeout");
    assert_eq!(driver.health(), IoDriverHealth::Ok);

    let error = driver
        .exchange_inputs(&mut inputs, 150)
        .expect_err("150 ms is past the timeout");
    assert_eq!(
        error.to_string(),
        "i/o freshness error 'can: no frame 0x181 for 100 ms'"
    );
    assert_eq!(driver.health(), faulted("no frame 0x181 for 100 ms"));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// Specification 11 §6.5: the next frame ends a receive's timeout. Catches a timeout kept
/// until a restart.
#[test]
fn the_next_frame_ends_a_receive_timeout() {
    let (mut driver, bus) = driver(OnError::Fault);
    let mut inputs = [0; 4];
    driver
        .exchange_inputs(&mut inputs, 150)
        .expect_err("150 ms is past the timeout");

    arrive(&bus, Received::Data(frame(RX, &[1, 2, 3, 4])));
    driver
        .exchange_inputs(&mut inputs, 160)
        .expect("the frame ends the timeout");

    assert_eq!(inputs, [1, 2, 3, 4]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// Specification 11 §6.5, `on_error` as for Modbus: under `warn` the resource keeps running
/// and the driver's health is degraded. Catches a resource faulted under `warn`.
#[test]
fn under_warn_a_timeout_degrades_the_driver_and_the_resource_runs_on() {
    let (mut driver, _bus) = driver(OnError::Warn);
    let mut inputs = [0; 4];

    driver
        .exchange_inputs(&mut inputs, 150)
        .expect("warn keeps the resource running");

    assert_eq!(driver.health(), degraded("no frame 0x181 for 100 ms"));
}

/// Specification 11 §6.5: a controller that goes bus-off makes the driver's health faulted
/// with `bus off`, on its output path too, as nothing can be sent, and it rejoins after the bus
/// recovery without a restart of the driver; the kernel reports the restart with
/// `CAN_ERR_RESTARTED`. Catches a bus-off ignored, one reported for the inputs alone (a
/// safe-state handoff confirmed while nothing can be sent), and one kept until a restart.
#[test]
fn bus_off_faults_the_driver_until_the_controller_restarts() {
    let (mut driver, bus) = driver(OnError::Fault);
    let mut inputs = [0; 4];
    arrive(
        &bus,
        Received::Error {
            bits: 0x40,
            data: [0; 8],
        },
    );

    let error = driver
        .exchange_inputs(&mut inputs, 10)
        .expect_err("bus-off fails the read");
    assert_eq!(error.to_string(), "i/o transport error 'can: bus off'");
    assert_eq!(driver.health(), faulted("bus off"));
    assert_eq!(driver.output_health(), faulted("bus off"));
    driver
        .exchange_outputs(&[0; 2], 10)
        .expect_err("bus-off fails the write");

    arrive(
        &bus,
        Received::Error {
            bits: 0x100,
            data: [0; 8],
        },
    );
    driver
        .exchange_inputs(&mut inputs, 20)
        .expect("the restarted controller reads");
    assert_eq!(driver.health(), IoDriverHealth::Ok);
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// Specification 11 §6.5: the controller rejoins after the bus recovery without a restart of
/// the driver, also when it recovers on its own without an error frame that says so, as an
/// MCP2515 with `restart-ms` does; the driver then reads the controller's state. A bus-off
/// stays while the state reads bus-off, and a bus-off is reported in the cycle that sees its
/// error frame even when the state already reads recovered. Catches a bus-off kept until a
/// restart, one cleared while the controller is still off the bus, and one never reported.
#[test]
fn a_controller_that_recovers_on_its_own_rejoins_when_its_state_does() {
    let (mut driver, bus) = driver(OnError::Warn);
    let mut inputs = [0; 4];
    let bus_off_frame = Received::Error {
        bits: 0x40,
        data: [0; 8],
    };
    arrive(&bus, bus_off_frame);
    bus.lock().expect("bus").state = Some(true);

    driver
        .exchange_inputs(&mut inputs, 10)
        .expect("warn keeps the resource running");
    assert_eq!(driver.health(), degraded("bus off"));
    driver
        .exchange_inputs(&mut inputs, 20)
        .expect("warn keeps the resource running");
    assert_eq!(driver.health(), degraded("bus off"));

    bus.lock().expect("bus").state = Some(false);
    driver
        .exchange_inputs(&mut inputs, 30)
        .expect("the recovered controller reads");
    assert_eq!(driver.health(), IoDriverHealth::Ok);

    arrive(&bus, bus_off_frame);
    driver
        .exchange_inputs(&mut inputs, 40)
        .expect("warn keeps the resource running");
    assert_eq!(driver.health(), degraded("bus off"));
}

/// `linux/can/error.h`: `CAN_ERR_BUSOFF` (0x40) is bus-off, `CAN_ERR_RESTARTED` (0x100) the
/// restart after it, and a controller problem (0x04) with `CAN_ERR_CRTL_ACTIVE` (0x40) in data
/// byte 1 the return to error-active; a warning (`CAN_ERR_CRTL_TX_WARNING`, 0x08) and a missing
/// acknowledgement (`CAN_ERR_ACK`, 0x20) change nothing. Catches a warning taken for the end
/// of bus-off, and a recovery missed.
#[test]
fn error_frames_show_bus_off_and_its_end() {
    let quiet = [0; 8];
    let active = [0, 0x40, 0, 0, 0, 0, 0, 0];
    let warning = [0, 0x08, 0, 0, 0, 0, 0, 0];

    assert_eq!(bus_off_change(0x40, &quiet), Some(true));
    assert_eq!(bus_off_change(0x100, &quiet), Some(false));
    assert_eq!(bus_off_change(0x04, &active), Some(false));
    assert_eq!(bus_off_change(0x04, &warning), None);
    assert_eq!(bus_off_change(0x20, &quiet), None);
}

/// Specification 11 §6.5: every `period_ms` the driver sends the frame with the `length` bytes
/// of the output image from `address`, and a transmit is due at once when the driver starts.
/// With 100 ms, the frame 0x201 with `AA BB` goes out at 0 ms and at 100 ms, not at 50 ms.
/// Catches a frame sent at every cycle, a first frame held back a period, and output bytes past
/// `length` sent.
#[test]
fn a_transmit_goes_out_at_once_and_then_every_period() {
    let (mut driver, bus) = driver(OnError::Fault);
    let outputs = [0xAA, 0xBB, 0xCC];

    driver.exchange_outputs(&outputs, 0).expect("send at once");
    driver
        .exchange_outputs(&outputs, 50)
        .expect("nothing is due");
    driver
        .exchange_outputs(&outputs, 100)
        .expect("send after a period");

    let expected = frame(TX, &[0xAA, 0xBB]);
    assert_eq!(bus.lock().expect("bus").sent, [expected, expected]);
}

/// Specification 11 §6.5 and the safe-state handoff confirmation: a frame the controller does
/// not take faults the output path under `on_error = "fault"`, so the write fails and a
/// safe-state handoff is not confirmed; a later handoff whose frames all go out makes it
/// healthy. Catches a lost frame counted as sent.
#[test]
fn a_frame_the_controller_does_not_take_faults_the_output_path() {
    let (mut driver, bus) = driver(OnError::Fault);
    let failure = "No buffer space available (os error 105)";
    bus.lock().expect("bus").send_failure = Some(failure.to_owned());

    let error = driver
        .exchange_outputs(&[0; 2], 0)
        .expect_err("a failed send fails the write");
    assert_eq!(
        error.to_string(),
        "i/o transport error 'can: No buffer space available (os error 105)'"
    );
    assert_eq!(driver.output_health(), faulted(failure));

    bus.lock().expect("bus").send_failure = None;
    driver
        .exchange_outputs(&[0; 2], 100)
        .expect("the next period goes out");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// Specification 11 §6.5: on Linux the interface's rate belongs to the operating system, and
/// the driver refuses another rate with both numbers; a virtual interface reports no rate and
/// has none to differ. Catches a driver that joins a 250 kbit/s bus at the 500 kbit/s of its
/// parameters.
#[test]
fn an_interface_at_another_bit_rate_is_refused_with_both_numbers() {
    let error = check_bitrate("can0", 500_000, Some(250_000)).expect_err("another rate");
    assert_eq!(
        error.to_string(),
        "invalid config 'can: the CAN interface can0 runs at 250000 bit/s, not the bitrate \
         500000 bit/s of its parameters'"
    );
    check_bitrate("can0", 500_000, Some(500_000)).expect("the same rate");
    check_bitrate("vcan0", 500_000, None).expect("a virtual interface");
}

/// Specification 11 §6.5 and 34 TARGET-01: the driver refuses, when it starts, an interface
/// that this computer lacks, that is not a CAN interface (`ARPHRD_CAN`, 280) or that is down
/// (`IFF_UP` clear in `0x80`; `0xc1` is up), with the driver's name and the reason. Catches a
/// socket opened on a down interface, which would never exchange a frame, and an Ethernet
/// interface taken for a CAN bus.
#[test]
fn an_interface_that_is_missing_not_can_or_down_is_refused() {
    let sysfs = FakeSysfs::new("can-interface");
    sysfs.set("class/net/can0/type", "280\n");
    sysfs.set("class/net/can0/flags", "0x80\n");
    sysfs.set("class/net/eth0/type", "1\n");
    sysfs.set("class/net/eth0/flags", "0x1003\n");
    let refusal = |interface: &str| match check_interface(sysfs.root(), interface) {
        Ok(()) => panic!("{interface} must be refused"),
        Err(error) => error.to_string(),
    };

    assert_eq!(
        refusal("can1"),
        "invalid config 'can: this computer has no CAN interface can1'"
    );
    assert_eq!(
        refusal("eth0"),
        "invalid config 'can: eth0 is not a CAN interface'"
    );
    assert_eq!(
        refusal("can0"),
        "invalid config 'can: the CAN interface can0 is down'"
    );
    assert_eq!(
        refusal("can0/x"),
        "invalid config 'can: line 'can0/x' is not a SocketCAN interface such as can0'"
    );

    sysfs.set("class/net/can0/flags", "0xc1\n");
    check_interface(sysfs.root(), "can0").expect("an up CAN interface");
}

/// Specification 11 §6.5: on Linux `tx` and `rx` are refused, as the interface's own
/// controller has its pins. Catches a chip's pins accepted and silently unused.
#[test]
fn tx_and_rx_are_refused_on_linux() {
    let mut with_tx = params(OnError::Fault);
    with_tx.tx = Some(SmolStr::new("GPIO4"));
    assert_eq!(
        check_pins(&with_tx).expect_err("tx").to_string(),
        "invalid config 'can: tx 'GPIO4' is refused on Linux, where the interface can0 has \
         its own pins'"
    );

    let mut with_rx = params(OnError::Fault);
    with_rx.rx = Some(SmolStr::new("GPIO5"));
    assert_eq!(
        check_pins(&with_rx).expect_err("rx").to_string(),
        "invalid config 'can: rx 'GPIO5' is refused on Linux, where the interface can0 has \
         its own pins'"
    );
}

/// `linux/can.h` and `CAN_RAW_FILTER`: a frame passes when its `can_id & can_mask` equals the
/// filter's. The filter of the standard 0x181 matches its 11 bits with the extended and the
/// remote flag clear; that of the extended 0x18FF0001 its 29 bits with the extended flag set.
/// Catches a filter that also passes the extended frame 0x181 or a remote frame.
#[test]
fn the_socket_filter_passes_the_data_frames_of_one_identifier() {
    assert_eq!(filter_words(RX), (0x181, 0xC000_07FF));
    assert_eq!(
        filter_words(CanId {
            id: 0x18FF_0001,
            extended: true,
        }),
        (0x98FF_0001, 0xDFFF_FFFF)
    );
}

/// CAN 2.0: an 11-bit identifier is at most 0x7FF and a 29-bit one at most 0x1FFFFFFF. A
/// frame goes to the socket with its identifier, its kind and its `len` bytes, and comes back
/// the same. Catches an extended frame sent as a standard one, data past `len` sent, and an
/// identifier cut down to fit.
#[test]
fn frames_cross_the_socket_unchanged() {
    let standard = frame(RX, &[1, 2]);
    let sent = socket_frame(&standard).expect("a standard frame");
    assert_eq!(sent.raw_id(), 0x181);
    assert!(!sent.is_extended());
    assert_eq!(sent.data(), [1, 2]);
    assert_eq!(data_frame(&sent), standard);

    let extended = frame(
        CanId {
            id: 0x18FF_0001,
            extended: true,
        },
        &[9],
    );
    let sent = socket_frame(&extended).expect("an extended frame");
    assert!(sent.is_extended());
    assert_eq!(data_frame(&sent), extended);

    let too_wide = frame(
        CanId {
            id: 0x800,
            extended: false,
        },
        &[],
    );
    assert!(socket_frame(&too_wide).is_none());
}
