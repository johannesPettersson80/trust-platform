//! The `can` driver's parameters and its exchange of frames with the process images (specification
//! 11 §6.5; CAN 2.0, ISO 11898-1): received frames write their data to input bytes, and output bytes
//! are sent as frames at their periods. The exchange here holds no controller: each runtime's
//! hardware layer hands it the frames it received and sends the frames it gives back.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

use super::{
    listing, parse_root, AddressKind, OnError, Once, ParamsError, Ranges, Table, LINE_ONCE,
};

/// The bit rates a `can` driver takes, those of `CiA 301`, in bit/s.
pub const BIT_RATES: [u32; 9] = [
    10_000, 20_000, 50_000, 100_000, 125_000, 250_000, 500_000, 800_000, 1_000_000,
];

/// The highest 11-bit identifier.
const MAX_STANDARD_ID: u32 = 0x7FF;

/// The highest 29-bit identifier, with `extended = true`.
const MAX_EXTENDED_ID: u32 = 0x1FFF_FFFF;

/// The most data bytes of a frame.
const MAX_LENGTH: u8 = 8;

/// The longest `timeout_ms` and `period_ms`.
const MAX_PERIOD_MS: u32 = 60_000;

/// What `line` must be.
const CONTROLLER: &str = "a non-empty string naming the CAN controller";

/// What `tx` and `rx` must be.
const PIN: &str = "a non-empty string naming the pin";

/// A receive's address: a `%IB` byte.
const RECEIVE_ADDRESS: AddressKind = AddressKind {
    area: IoArea::Input,
    size: IoSize::Byte,
    prefix: "%IB",
    noun: "byte",
};

/// A transmit's address: a `%QB` byte.
const TRANSMIT_ADDRESS: AddressKind = AddressKind {
    area: IoArea::Output,
    size: IoSize::Byte,
    prefix: "%QB",
    noun: "byte",
};

/// The `can` driver's parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanParams {
    /// `line`: a chip's controller such as `TWAI0`, or a Linux `SocketCAN` interface such as
    /// `can0`.
    pub line: SmolStr,
    /// `tx`: the pin that goes to the transceiver's input, on a chip; refused on Linux.
    pub tx: Option<SmolStr>,
    /// `rx`: the pin that comes from the transceiver's output, on a chip; refused on Linux.
    pub rx: Option<SmolStr>,
    /// `bitrate`, one of [`BIT_RATES`]; required.
    pub bitrate: u32,
    /// `receive`, in `io.toml` order.
    pub receive: Vec<CanReceive>,
    /// `transmit`, in `io.toml` order.
    pub transmit: Vec<CanTransmit>,
    /// `on_error`; default `fault`.
    pub on_error: OnError,
}

/// A frame identifier: 11 bits, or 29 bits when `extended`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanId {
    /// `id`.
    pub id: u32,
    /// `extended`; default false.
    pub extended: bool,
}

/// One frame the driver receives into the input image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanReceive {
    /// `id` and `extended`.
    pub id: CanId,
    /// `address`, a concrete `%IB` byte: the first of `length` input bytes.
    pub address: IoAddress,
    /// `length`, 0 to 8.
    pub length: u8,
    /// `timeout_ms`, 1 to 60,000: the longest time without the frame before the driver's health is
    /// faulted; `None` without a timeout.
    pub timeout_ms: Option<u32>,
}

/// One frame the driver sends from the output image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanTransmit {
    /// `id` and `extended`.
    pub id: CanId,
    /// `address`, a concrete `%QB` byte: the first of `length` output bytes.
    pub address: IoAddress,
    /// `length`, 0 to 8.
    pub length: u8,
    /// `period_ms`, 1 to 60,000; required.
    pub period_ms: u32,
}

/// A data frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanFrame {
    /// The identifier.
    pub id: CanId,
    /// The data length code's bytes, 0 to 8.
    pub len: u8,
    /// The data; the bytes from `len` on are 0.
    pub data: [u8; 8],
}

impl CanParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 11 §6.5.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(
            &[
                "bitrate", "line", "on_error", "receive", "rx", "transmit", "tx",
            ],
            "the can driver",
        )?;
        let mut lines = Once::new(LINE_ONCE);
        let line = root.required_line("line", CONTROLLER, &mut lines)?;
        let tx = root.line("tx", PIN, &mut lines)?;
        let rx = root.line("rx", PIN, &mut lines)?;
        if tx.is_some() != rx.is_some() {
            let (missing, given) = if tx.is_some() {
                ("rx", "tx")
            } else {
                ("tx", "rx")
            };
            return Err(root.refuse(
                missing,
                format!(
                    "is required with {given}: a chip's controller takes both pins, Linux neither"
                ),
            ));
        }
        let bitrate = bitrate(&root)?;
        let receive = receives(&root)?;
        let transmit = transmits(&root)?;
        let on_error = on_error(&root)?;
        Ok(Self {
            line,
            tx,
            rx,
            bitrate,
            receive,
            transmit,
            on_error,
        })
    }
}

/// `on_error`, read as the Modbus driver reads it: trimmed, in any case, with `warning` for `warn`;
/// `fault` when absent (specification 11 §6.5).
fn on_error(root: &Table<'_>) -> Result<OnError, ParamsError> {
    let Some(value) = root.get("on_error") else {
        return Ok(OnError::default());
    };
    match value
        .as_str()
        .map(|text| text.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("fault") => Ok(OnError::Fault),
        Some("warn" | "warning") => Ok(OnError::Warn),
        Some("ignore") => Ok(OnError::Ignore),
        _ => Err(root.refuse(
            "on_error",
            format!("must be \"fault\", \"warn\" or \"ignore\", not {value}"),
        )),
    }
}

/// `bitrate`, one of the rates of `CiA 301`; required.
fn bitrate(root: &Table<'_>) -> Result<u32, ParamsError> {
    let rates: Vec<String> = BIT_RATES.iter().map(ToString::to_string).collect();
    let what = format!("one of the CiA 301 bit rates {}", listing(&rates, " or "));
    let Some(value) = root.get("bitrate") else {
        return Err(root.refuse("bitrate", format!("is required, {what}")));
    };
    value
        .as_u64()
        .and_then(|rate| u32::try_from(rate).ok())
        .filter(|rate| BIT_RATES.contains(rate))
        .ok_or_else(|| root.refuse("bitrate", format!("must be {what}, not {value}")))
}

/// The `receive` list: each entry's identifier and its address once in the list.
fn receives(root: &Table<'_>) -> Result<Vec<CanReceive>, ParamsError> {
    let mut ids = Once::new("two receives with one id are refused");
    let mut bytes = Ranges::new();
    let mut receive = Vec::new();
    for entry in root.list("receive")? {
        entry.only(
            &["address", "extended", "id", "length", "timeout_ms"],
            "a receive",
        )?;
        let id = can_id(&entry, &mut ids)?;
        let address = entry.address(&RECEIVE_ADDRESS)?;
        let length = entry.required_integer("length", 0, MAX_LENGTH)?;
        bytes.claim(&entry, &address, u32::from(length), RECEIVE_ADDRESS.prefix)?;
        let timeout_ms = entry.integer("timeout_ms", 1, MAX_PERIOD_MS)?;
        receive.push(CanReceive {
            id,
            address,
            length,
            timeout_ms,
        });
    }
    Ok(receive)
}

/// The `transmit` list: each entry's identifier and its address once in the list.
fn transmits(root: &Table<'_>) -> Result<Vec<CanTransmit>, ParamsError> {
    let mut ids = Once::new("two transmits with one id are refused");
    let mut bytes = Ranges::new();
    let mut transmit = Vec::new();
    for entry in root.list("transmit")? {
        entry.only(
            &["address", "extended", "id", "length", "period_ms"],
            "a transmit",
        )?;
        let id = can_id(&entry, &mut ids)?;
        let address = entry.address(&TRANSMIT_ADDRESS)?;
        let length = entry.required_integer("length", 0, MAX_LENGTH)?;
        bytes.claim(&entry, &address, u32::from(length), TRANSMIT_ADDRESS.prefix)?;
        let period_ms = entry.required_integer("period_ms", 1, MAX_PERIOD_MS)?;
        transmit.push(CanTransmit {
            id,
            address,
            length,
            period_ms,
        });
    }
    Ok(transmit)
}

/// An entry's `id` and `extended`: 11 bits, or 29 bits with `extended = true`; refused when an
/// earlier entry of its list has the same identifier (`ids`).
fn can_id(entry: &Table<'_>, ids: &mut Once<CanId>) -> Result<CanId, ParamsError> {
    let extended = entry.boolean("extended")?.unwrap_or(false);
    let (max, what) = if extended {
        (MAX_EXTENDED_ID, "a 29-bit identifier from 0 to 0x1fffffff")
    } else {
        (
            MAX_STANDARD_ID,
            "an 11-bit identifier from 0 to 0x7ff, or to 0x1fffffff with extended = true",
        )
    };
    let Some(value) = entry.get("id") else {
        return Err(entry.refuse("id", format!("is required, {what}")));
    };
    let Some(id) = value
        .as_u64()
        .and_then(|id| u32::try_from(id).ok())
        .filter(|id| *id <= max)
    else {
        let given = value
            .as_u64()
            .map_or_else(|| value.to_string(), |id| format!("{id} ({id:#x})"));
        return Err(entry.refuse("id", format!("must be {what}, not {given}")));
    };
    let id = CanId { id, extended };
    ids.claim(id, entry.key("id"), &id_text(id))?;
    Ok(id)
}

/// An identifier as a refusal and a timeout write it, in hexadecimal: `0x181`, and an extended one
/// with all eight digits, `0x00000181`, as `candump` writes it, so that it never reads as the
/// 11-bit identifier of the same value.
fn id_text(id: CanId) -> String {
    if id.extended {
        format!("{:#010x}", id.id)
    } else {
        format!("{:#x}", id.id)
    }
}

/// The driver's exchange of frames with the process images: the newest data of each receive, each
/// receive's timeout, and when each transmit is due. Times are milliseconds of a monotonic clock.
#[derive(Debug, Clone)]
pub struct CanExchange {
    params: CanParams,
    /// Each receive's newest frame and when it came, in the order of `params.receive`.
    receives: Vec<Received>,
    /// When each transmit is due next, in the order of `params.transmit`.
    due_ms: Vec<u64>,
}

/// A receive's newest frame, and the time its timeout runs from.
#[derive(Debug, Clone, Copy)]
struct Received {
    /// The newest frame; `None` before the first.
    frame: Option<CanFrame>,
    /// When the newest frame came, or when the exchange started, before the first frame.
    since_ms: u64,
}

impl CanExchange {
    /// The exchange of `params`, started at `now_ms`: no frame received yet, so a receive's timeout
    /// runs from `now_ms`, and every transmit is due at once.
    #[must_use]
    pub fn new(params: CanParams, now_ms: u64) -> Self {
        let waiting = Received {
            frame: None,
            since_ms: now_ms,
        };
        let receives = alloc::vec![waiting; params.receive.len()];
        let due_ms = alloc::vec![now_ms; params.transmit.len()];
        Self {
            params,
            receives,
            due_ms,
        }
    }

    /// Takes a frame the controller received at `now_ms`: a frame of a receive's identifier becomes
    /// that receive's newest data; any other frame is dropped.
    pub fn received(&mut self, frame: &CanFrame, now_ms: u64) {
        for (receive, state) in self.params.receive.iter().zip(&mut self.receives) {
            if receive.id == frame.id {
                *state = Received {
                    frame: Some(*frame),
                    since_ms: now_ms,
                };
            }
        }
    }

    /// Writes each receive's newest data to its bytes of `inputs`, the input image, zero-filled past
    /// the frame's length, and leaves the bytes of a receive that has had no frame as they are. A
    /// byte past the end of `inputs` is left out: no binding reads it.
    ///
    /// # Errors
    ///
    /// `no frame <id> for <timeout_ms> ms` for the first receive whose timeout ran out at `now_ms`,
    /// with the identifier in hexadecimal as `0x181` (an extended one with all eight digits,
    /// `0x00000181`); the inputs are written all the same. A timeout runs out when no frame came
    /// for `timeout_ms` or longer.
    pub fn read_inputs(&mut self, inputs: &mut [u8], now_ms: u64) -> Result<(), String> {
        let mut timed_out = None;
        for (receive, state) in self.params.receive.iter().zip(&self.receives) {
            if let Some(frame) = &state.frame {
                write_data(inputs, receive, frame);
            }
            if let Some(timeout_ms) = receive.timeout_ms
                && timed_out.is_none()
                && now_ms.saturating_sub(state.since_ms) >= u64::from(timeout_ms)
            {
                timed_out = Some(format!(
                    "no frame {} for {timeout_ms} ms",
                    id_text(receive.id)
                ));
            }
        }
        match timed_out {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Gives `send` each transmit due at `now_ms`, with its bytes of `outputs`, the output image (a
    /// byte past the end of `outputs` is sent as 0), and sets its next time one period after the
    /// time it was due. A transmit late by a period or more, whose next time would not be after
    /// `now_ms`, is due once and then keeps its period from `now_ms`.
    pub fn send_due(&mut self, outputs: &[u8], now_ms: u64, mut send: impl FnMut(CanFrame)) {
        for (transmit, due_ms) in self.params.transmit.iter().zip(&mut self.due_ms) {
            if now_ms < *due_ms {
                continue;
            }
            send(frame_of(transmit, outputs));
            let period_ms = u64::from(transmit.period_ms);
            let next_ms = due_ms.saturating_add(period_ms);
            *due_ms = if next_ms > now_ms {
                next_ms
            } else {
                now_ms.saturating_add(period_ms)
            };
        }
    }
}

/// Writes `frame`'s data to the `length` input bytes of `receive`, with zeros where the frame's
/// data length code does not reach; a byte past the end of `inputs` is left out.
fn write_data(inputs: &mut [u8], receive: &CanReceive, frame: &CanFrame) {
    let Ok(start) = usize::try_from(receive.address.byte) else {
        return;
    };
    let Some(window) = inputs.get_mut(start..) else {
        return;
    };
    let carried = usize::from(frame.len);
    let data = frame
        .data
        .iter()
        .enumerate()
        .map(|(index, byte)| if index < carried { *byte } else { 0 });
    for (slot, byte) in window
        .iter_mut()
        .zip(data)
        .take(usize::from(receive.length))
    {
        *slot = byte;
    }
}

/// The frame `transmit` sends: its identifier, its length and its bytes of `outputs`, 0 past the
/// end of `outputs` and past its length.
fn frame_of(transmit: &CanTransmit, outputs: &[u8]) -> CanFrame {
    let len = transmit.length.min(MAX_LENGTH);
    let mut data = [0; 8];
    let start = usize::try_from(transmit.address.byte).unwrap_or(usize::MAX);
    if let Some(window) = outputs.get(start..) {
        for (byte, value) in data.iter_mut().zip(window).take(usize::from(len)) {
            *byte = *value;
        }
    }
    CanFrame {
        id: transmit.id,
        len,
        data,
    }
}

#[cfg(test)]
mod tests;
