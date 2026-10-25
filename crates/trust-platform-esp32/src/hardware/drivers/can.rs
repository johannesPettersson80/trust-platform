//! The `can` driver on the C6 (specification 11 §6.5; CAN 2.0, ISO 11898-1; specification 34
//! Appendix D.2): one of the two TWAI controllers, `TWAI0` or `TWAI1`, whose `tx` and `rx` the GPIO
//! matrix routes to the pins that go to the transceiver. The controller runs in normal mode at the
//! driver's bit rate, from the C6's 40 MHz crystal, and its acceptance filter keeps the data frames
//! of the receive identifiers when they share one frame format.
//!
//! At the start of each cycle the driver hands the frames the controller received since the last one
//! to the shared exchange, which writes the newest data of each receive into its input bytes and
//! checks the receive timeouts. After the tasks it gives the exchange the output image, which hands
//! back the frames that are due, and the controller takes them in order until its one transmit
//! buffer is busy; a frame it cannot take now is sent at a later cycle, the newest data of each
//! identifier replacing the older. Nothing waits for the bus. The controller's receive FIFO holds
//! 64 bytes, five standard or four extended frames of eight data bytes, so at most that many frames
//! that pass the filter may arrive between two cycle starts, and its one transmit buffer takes one
//! frame per cycle.
//!
//! A controller that goes bus-off makes the driver's health faulted with `bus off`; the driver starts
//! the bus recovery of ISO 11898-1 (128 occurrences of 11 recessive bits), after which the
//! controller rejoins by itself, without a restart.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;

use esp_hal::gpio::{Flex, InputConfig, InputSignal, Level, NoPin, OutputConfig, OutputSignal};
use esp_hal::twai::filter::{SingleExtendedFilter, SingleStandardFilter};
use esp_hal::twai::{
    AnyTwai, BaudRate, EspTwaiFrame, ExtendedId, Id, StandardId, TimingConfig, Twai,
    TwaiConfiguration, TwaiMode,
};
use esp_hal::Blocking;
use smol_str::SmolStr;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::can::{
    CanExchange, CanFrame, CanId, CanParams, CanReceive,
};
use trust_runtime_services::io_drivers::OnError;
use trust_runtime_services::status::IoDriverHealth;

use super::{refusal, Lease, PeripheralSlot};
use crate::hardware::clock::now_us;
use crate::process_io::{GpioBank, PinClaim};

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = "can";

/// The clock of the C6's TWAI controllers: the 40 MHz crystal (esp-hal 1.2.2, `BaudRate`).
const CLOCK_HZ: u32 = 40_000_000;

/// The most frames one cycle start takes from the controller: more than its FIFO holds, a bound for
/// the scan.
const DRAIN_LIMIT: usize = 64;

/// The error count from which a controller is error passive (ISO 11898-1: above 127).
const ERROR_PASSIVE: u8 = 128;

/// The health of a controller that is bus-off, the specification's text.
const BUS_OFF: &str = "bus off";

/// The health of a receive FIFO that overran and lost frames.
const OVERRUN: &str = "receive overrun: frames were lost";

/// The health of a controller one of whose error counts reached 128, as when no node acknowledges
/// its frames.
const PASSIVE: &str = "error passive: 128 or more bus errors";

/// The error of a driver whose controller was stopped, which only its drop does.
const STOPPED: &str = "the controller is stopped";

/// The `can` driver: one TWAI controller, its pins and the exchange of frames with the images.
struct CanDriver {
    /// The running controller, over a copy of the peripheral the lease holds; taken and stopped when
    /// the driver is dropped.
    twai: Option<Twai<'static, Blocking>>,
    exchange: CanExchange,
    /// The frames that are due and that the controller has not taken yet, one for each identifier,
    /// in the order they fell due. It never holds more than the driver's transmits, for which it
    /// has room from the start.
    pending: VecDeque<CanFrame>,
    /// 0 for TWAI0, 1 for TWAI1.
    controller: usize,
    on_error: OnError,
    /// Whether the bus recovery of the present bus-off has been started.
    recovering: bool,
    /// The health of the inputs' path after their last exchange.
    inputs: IoDriverHealth,
    /// The health of the outputs' path after their last exchange.
    outputs: IoDriverHealth,
    /// The `tx` pin: a GPIO output whose latch holds the recessive level, high, which the GPIO
    /// matrix routes the controller's transmit signal over.
    tx: Flex<'static>,
    /// The `rx` pin, an input the GPIO matrix routes to the controller.
    _rx: Flex<'static>,
    _tx_claim: PinClaim,
    _rx_claim: PinClaim,
    /// Declared last: the controller and the pins are let go before the peripheral is free again.
    _controller: Lease<AnyTwai<'static>>,
}

impl Drop for CanDriver {
    /// The controller goes to reset mode, which leaves its transmit output recessive; the `tx` pin
    /// leaves it for its latch, which holds the recessive level, and the controller's receive input
    /// reads a recessive level instead of the `rx` pin.
    fn drop(&mut self) {
        if let Some(twai) = self.twai.take() {
            drop(twai.stop());
        }
        transmit_signal(self.controller).disconnect_from(&self.tx);
        receive_signal(self.controller).connect_to(&Level::High);
    }
}

/// Builds the `can` driver of the parameters `params` (JSON): the controller `line` is claimed from
/// `controllers`, its `tx` and `rx` pins from `bank`, and it starts in normal mode at `bitrate`.
///
/// # Errors
///
/// - the shared parameter refusal, `can: <key>: <reason>`;
/// - `can: line: <line> is not a CAN controller of the C6: its controllers are TWAI0 and TWAI1`;
/// - `can: bitrate: <line> cannot run at <rate> bit/s from its 40000000 Hz clock`;
/// - `can: tx: <line> needs the pin that goes to the transceiver's input`, and for `rx` the pin
///   that comes from the transceiver's output;
/// - `can: line: <line> is already used by another can driver`;
/// - `can: tx: <reason>` or `can: rx: <reason>` for a pin that is not a pin name, that the chip
///   lacks or keeps, or that another line holds, the other of the two included.
pub(super) fn claim(
    params: &str,
    bank: &GpioBank,
    controllers: &[PeripheralSlot<AnyTwai<'static>>; 2],
) -> Result<Box<dyn ChipDriver>, String> {
    let params = CanParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    let line = params.line.clone();
    let (controller, slot) = match line.as_str() {
        "TWAI0" => (0, &controllers[0]),
        "TWAI1" => (1, &controllers[1]),
        _ => {
            return Err(refusal(
                NAME,
                "line",
                format!(
                    "{line} is not a CAN controller of the C6: its controllers are TWAI0 and TWAI1"
                ),
            ));
        }
    };
    let bit_timing = timing(params.bitrate).ok_or_else(|| {
        refusal(
            NAME,
            "bitrate",
            format!(
                "{line} cannot run at {} bit/s from its {CLOCK_HZ} Hz clock",
                params.bitrate
            ),
        )
    })?;
    let tx_name = params.tx.clone().ok_or_else(|| {
        refusal(
            NAME,
            "tx",
            format!("{line} needs the pin that goes to the transceiver's input"),
        )
    })?;
    let rx_name = params.rx.clone().ok_or_else(|| {
        refusal(
            NAME,
            "rx",
            format!("{line} needs the pin that comes from the transceiver's output"),
        )
    })?;
    let lease = slot.claim().ok_or_else(|| {
        refusal(
            NAME,
            "line",
            format!("{line} is already used by another can driver"),
        )
    })?;
    let (_, tx_pin, tx_claim) = bank
        .claim_pin(&tx_name)
        .map_err(|error| refusal(NAME, "tx", error))?;
    let (_, rx_pin, rx_claim) = bank
        .claim_pin(&rx_name)
        .map_err(|error| refusal(NAME, "rx", error))?;
    // The transmit pin drives the recessive level, high, until the controller drives it, and again
    // when the driver lets it go.
    let mut tx = Flex::new(tx_pin);
    tx.set_level(Level::High);
    tx.apply_output_config(&OutputConfig::default());
    tx.set_output_enable(true);
    let mut rx = Flex::new(rx_pin);
    rx.apply_input_config(&InputConfig::default());
    rx.set_input_enable(true);
    // SAFETY: `PeripheralSlot::claim` refused this controller to every other driver until `lease`
    // is dropped, and the driver below keeps the controller built from this copy in a field it
    // stops and drops in `Drop`, before its lease field is dropped, so the copy is gone before the
    // controller can be claimed again. At most one instance of the controller is in use at a time,
    // which `clone_unchecked` asks of its caller.
    let peripheral = unsafe { lease.get().clone_unchecked() };
    // The controller is given no transmit pin: the driver routes its transmit signal over `tx`
    // itself, so that it can give the pin back when it is dropped.
    let mut configuration = TwaiConfiguration::new(
        peripheral,
        rx.peripheral_input(),
        NoPin,
        BaudRate::Custom(bit_timing),
        TwaiMode::Normal,
    );
    accept(&mut configuration, &params.receive);
    transmit_signal(controller).connect_to(&tx);
    let twai = configuration.start();
    let on_error = params.on_error;
    let pending = VecDeque::with_capacity(params.transmit.len());
    let exchange = CanExchange::new(params, now_ms());
    Ok(Box::new(CanDriver {
        twai: Some(twai),
        exchange,
        pending,
        controller,
        on_error,
        recovering: false,
        inputs: IoDriverHealth::Ok,
        outputs: IoDriverHealth::Ok,
        tx,
        _rx: rx,
        _tx_claim: tx_claim,
        _rx_claim: rx_claim,
        _controller: lease,
    }))
}

/// The bit timing of `bitrate` on the C6's TWAI: a time quantum of `quantum` cycles of its 40 MHz
/// clock, and a bit of 1 + `tseg_1` + `tseg_2` quanta sampled after 1 + `tseg_1` of them, with a
/// resynchronization jump of 3 quanta. These are ESP-IDF's timings for the C6: 20 quanta sampled at
/// 80 %, and at 800,000 bit/s, where a bit is 50 clock cycles, 25 quanta of 2 cycles sampled at
/// 68 %. For example 10,000 bit/s: 200 cycles × 20 quanta = 4,000 cycles of 40 MHz, 100 µs.
/// `None` for a rate the table does not hold.
fn timing(bitrate: u32) -> Option<TimingConfig> {
    let (quantum, tseg_1, tseg_2): (u16, u8, u8) = match bitrate {
        10_000 => (200, 15, 4),
        20_000 => (100, 15, 4),
        50_000 => (40, 15, 4),
        100_000 => (20, 15, 4),
        125_000 => (16, 15, 4),
        250_000 => (8, 15, 4),
        500_000 => (4, 15, 4),
        800_000 => (2, 16, 8),
        1_000_000 => (2, 15, 4),
        _ => return None,
    };
    Some(TimingConfig {
        // esp-hal 1.2.2 halves the prescaler on the C6, whose clock is half the 80 MHz its timings
        // assume, so it is given twice the quantum.
        baud_rate_prescaler: quantum * 2,
        sync_jump_width: 3,
        tseg_1,
        tseg_2,
        triple_sample: false,
    })
}

/// Sets the acceptance filter of `configuration` to the data frames of the `receives`'
/// identifiers when they are all standard or all extended: the identifier bits on which they agree
/// must match, and the others may be anything. The filter lets more frames through than the
/// receives name, and the exchange drops those. With no receive, or with both formats, every frame
/// passes, as the controller's default filter lets them.
fn accept(configuration: &mut TwaiConfiguration<'static, Blocking>, receives: &[CanReceive]) {
    let Some(first) = receives.first() else {
        return;
    };
    let differ = receives
        .iter()
        .fold(0, |differ, receive| differ | (receive.id.id ^ first.id.id));
    if receives.iter().all(|receive| !receive.id.extended) {
        let code = u16::try_from(first.id.id).ok().and_then(StandardId::new);
        let mask = u16::try_from(!differ & 0x7FF)
            .ok()
            .and_then(StandardId::new);
        if let (Some(code), Some(mask)) = (code, mask) {
            configuration.set_filter(SingleStandardFilter::new_from_code_mask(
                code,
                mask,
                false,
                true,
                [0, 0],
                [0, 0],
            ));
        }
    } else if receives.iter().all(|receive| receive.id.extended) {
        let code = ExtendedId::new(first.id.id);
        let mask = ExtendedId::new(!differ & 0x1FFF_FFFF);
        if let (Some(code), Some(mask)) = (code, mask) {
            configuration.set_filter(SingleExtendedFilter::new_from_code_mask(
                code, mask, false, true,
            ));
        }
    }
}

/// The GPIO matrix's transmit signal of TWAI `controller`.
fn transmit_signal(controller: usize) -> OutputSignal {
    if controller == 0 {
        OutputSignal::TWAI0_TX
    } else {
        OutputSignal::TWAI1_TX
    }
}

/// The GPIO matrix's receive signal of TWAI `controller`.
fn receive_signal(controller: usize) -> InputSignal {
    if controller == 0 {
        InputSignal::TWAI0_RX
    } else {
        InputSignal::TWAI1_RX
    }
}

/// Milliseconds since boot, from SYSTIMER: the exchange's clock.
fn now_ms() -> u64 {
    now_us() / 1_000
}

/// Whether the controller is bus-off. The first check that finds it so starts the bus recovery of
/// ISO 11898-1, which the controller completes by itself once the bus has been idle for 128 times 11
/// bits; starting it again at every cycle would start the count again.
fn bus_off(twai: &mut Twai<'static, Blocking>, recovering: &mut bool) -> bool {
    if twai.is_bus_off() {
        if !*recovering {
            twai.initiate_recovery();
            *recovering = true;
        }
        true
    } else {
        *recovering = false;
        false
    }
}

/// Hands `exchange` the data frames the controller received, at most `DRAIN_LIMIT`, as received at
/// `now_ms`. Returns whether the receive FIFO overran: frames were lost, and the FIFO is emptied so
/// that the controller receives again.
fn drain(twai: &mut Twai<'static, Blocking>, exchange: &mut CanExchange, now_ms: u64) -> bool {
    for _ in 0..DRAIN_LIMIT {
        if twai.num_available_messages() == 0 {
            return false;
        }
        match twai.receive() {
            Ok(frame) => {
                if let Some(frame) = can_frame(&frame) {
                    exchange.received(&frame, now_ms);
                }
            }
            // The controller holds frames and is not bus-off, so its one error left is an overrun:
            // the frame at the head of the FIFO is incomplete.
            Err(_) => {
                twai.clear_receive_fifo();
                return true;
            }
        }
    }
    false
}

/// Hands the controller the pending frames in order until it takes no more: its one transmit buffer
/// holds a frame until the bus has carried it, so the frames left wait for the next cycle.
fn transmit(twai: &mut Twai<'static, Blocking>, pending: &mut VecDeque<CanFrame>) {
    while let Some(frame) = pending.front() {
        // The parameters' identifiers fit their formats and their lengths 8 bytes, so every frame
        // converts; one that did not could never be sent, and is dropped.
        if let Some(frame) = esp_frame(frame)
            && twai.transmit(&frame).is_err()
        {
            return;
        }
        pending.pop_front();
    }
}

/// Puts `frame` among the pending frames: it replaces the pending frame of its identifier, whose
/// data it brings up to date, or is added after the others.
fn queue(pending: &mut VecDeque<CanFrame>, frame: CanFrame) {
    match pending.iter_mut().find(|queued| queued.id == frame.id) {
        Some(queued) => *queued = frame,
        None => pending.push_back(frame),
    }
}

/// The data frame `frame` the controller received, for the exchange; `None` for a remote frame,
/// which carries no data.
fn can_frame(frame: &EspTwaiFrame) -> Option<CanFrame> {
    if frame.is_remote_request() {
        return None;
    }
    let id = match frame.identifier() {
        Id::Standard(id) => CanId {
            id: u32::from(id.as_raw()),
            extended: false,
        },
        Id::Extended(id) => CanId {
            id: id.as_raw(),
            extended: true,
        },
    };
    // The controller gives at most 8 data bytes, also for a data length code above 8.
    let payload = frame.data();
    let len = payload.len().min(8);
    let mut data = [0; 8];
    data.get_mut(..len)?.copy_from_slice(payload.get(..len)?);
    Some(CanFrame {
        id,
        len: u8::try_from(len).ok()?,
        data,
    })
}

/// The controller's data frame of `frame`; `None` for an identifier its format cannot hold.
fn esp_frame(frame: &CanFrame) -> Option<EspTwaiFrame> {
    let data = frame.data.get(..usize::from(frame.len.min(8)))?;
    if frame.id.extended {
        EspTwaiFrame::new(ExtendedId::new(frame.id.id)?, data)
    } else {
        EspTwaiFrame::new(StandardId::new(u16::try_from(frame.id.id).ok()?)?, data)
    }
}

/// The health of one path faulted with `error`.
fn faulted(error: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: error.into(),
    }
}

/// The health of one path degraded with the fixed text `error`, without an allocation.
fn degraded(error: &'static str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: SmolStr::new_static(error),
    }
}

/// The driver's health: the worse of its two paths, faulted before degraded before ok, and the
/// inputs' path when both are as bad.
fn worse(inputs: &IoDriverHealth, outputs: &IoDriverHealth) -> IoDriverHealth {
    let rank = |health: &IoDriverHealth| match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    };
    if rank(outputs) > rank(inputs) {
        outputs.clone()
    } else {
        inputs.clone()
    }
}

impl ChipDriver for CanDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Takes the frames the controller received into the exchange, then writes each receive's
    /// newest data into its input bytes. A bus-off controller is the error `bus off`, and starts
    /// its recovery; otherwise the first receive whose timeout ran out is the error. A FIFO that
    /// overran degrades the inputs' path.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), String> {
        let now = now_ms();
        let Some(twai) = self.twai.as_mut() else {
            self.inputs = faulted(STOPPED);
            return Err(STOPPED.into());
        };
        let off = bus_off(twai, &mut self.recovering);
        let overrun = !off && drain(twai, &mut self.exchange, now);
        let timeouts = self.exchange.read_inputs(image, now);
        let (result, health) = if off {
            (Err(String::from(BUS_OFF)), faulted(BUS_OFF))
        } else {
            match timeouts {
                Err(error) => {
                    let health = faulted(&error);
                    (Err(error), health)
                }
                Ok(()) if overrun => (Ok(()), degraded(OVERRUN)),
                Ok(()) => (Ok(()), IoDriverHealth::Ok),
            }
        };
        self.inputs = health;
        result
    }

    /// Queues the frames the exchange finds due in the output image, then hands the controller the
    /// pending frames until it takes no more. A bus-off controller is the error `bus off`, and
    /// starts its recovery; the frames wait. An error-passive controller degrades the outputs' path.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), String> {
        let now = now_ms();
        let pending = &mut self.pending;
        self.exchange
            .send_due(image, now, |frame| queue(pending, frame));
        let Some(twai) = self.twai.as_mut() else {
            self.outputs = faulted(STOPPED);
            return Err(STOPPED.into());
        };
        if bus_off(twai, &mut self.recovering) {
            self.outputs = faulted(BUS_OFF);
            return Err(BUS_OFF.into());
        }
        transmit(twai, &mut self.pending);
        let passive = twai.transmit_error_count() >= ERROR_PASSIVE
            || twai.receive_error_count() >= ERROR_PASSIVE;
        self.outputs = if passive {
            degraded(PASSIVE)
        } else {
            IoDriverHealth::Ok
        };
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        worse(&self.inputs, &self.outputs)
    }

    fn on_error(&self) -> OnError {
        self.on_error
    }
}
