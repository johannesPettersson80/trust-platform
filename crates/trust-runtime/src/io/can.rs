//! The `can` driver of a computer (specification 11 §6.5; CAN 2.0, ISO 11898-1). On Linux the line
//! is a SocketCAN interface such as `can0`, whose bit rate belongs to the operating system: the
//! driver refuses an interface that is down or that runs at another rate than its `bitrate`, then
//! exchanges frames over a raw socket at each cycle boundary without waiting. A controller that
//! goes bus-off reports it with an error frame, and its restart or its return to error-active with
//! another; a controller that recovers on its own without saying so is found recovered through its
//! state, which the driver reads over netlink while the bus is off. The interface's `restart-ms`
//! decides when Linux restarts a controller after bus-off. The parameters and the
//! exchange of frames with the process images are the shared ones of
//! `trust_runtime_services::io_drivers::can`; this module holds the interface checks, the socket
//! (the `socketcan` crate) and the driver's health under `on_error`.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use smol_str::SmolStr;
use socketcan::nl::CanState;
use socketcan::{
    CanDataFrame, CanFilter, CanInterface, CanSocket, EmbeddedFrame, ExtendedId, Frame, Id, Socket,
    SocketOptions, StandardId,
};
use trust_runtime_services::io_drivers::can::{CanExchange, CanFrame, CanId, CanParams};
use trust_runtime_services::io_drivers::OnError;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name, which starts each of its errors.
const NAME: &str = "can";

/// The folder of the network interfaces under the sysfs root.
const NETWORK_INTERFACES: &str = "class/net";

/// `ARPHRD_CAN`, the hardware type of a CAN interface (`linux/if_arp.h`).
const ARPHRD_CAN: &str = "280";

/// `IFF_UP` in an interface's flags: the interface is up (`linux/if.h`).
const IFF_UP: u32 = 0x1;

/// The longest name of a network interface: `IFNAMSIZ` less its terminating zero.
const INTERFACE_NAME_MAX: usize = 15;

/// `CAN_EFF_FLAG`: the frame has a 29-bit identifier (`linux/can.h`).
const EFF_FLAG: u32 = 0x8000_0000;
/// `CAN_RTR_FLAG`: a remote frame.
const RTR_FLAG: u32 = 0x4000_0000;
/// `CAN_SFF_MASK`: the bits of an 11-bit identifier.
const SFF_MASK: u32 = 0x0000_07FF;
/// `CAN_EFF_MASK`: the bits of a 29-bit identifier.
const EFF_MASK: u32 = 0x1FFF_FFFF;

/// `CAN_RAW_FILTER_MAX`, the most filters a raw socket takes (`linux/can/raw.h`).
const FILTER_MAX: usize = 512;

/// `CAN_ERR_CRTL`: a controller problem, detailed in data byte 1 (`linux/can/error.h`).
const ERR_CONTROLLER: u32 = 0x0000_0004;
/// `CAN_ERR_BUSOFF`: the controller went bus-off.
const ERR_BUS_OFF: u32 = 0x0000_0040;
/// `CAN_ERR_RESTARTED`: the controller restarted after bus-off.
const ERR_RESTARTED: u32 = 0x0000_0100;
/// `CAN_ERR_CRTL_ACTIVE` in data byte 1: the controller is error-active again.
const CONTROLLER_ACTIVE: u8 = 0x40;

/// The health of a controller that is bus-off (specification 11 §6.5).
const BUS_OFF: &str = "bus off";

/// The most frames one cycle takes from the socket. A raw socket's queue holds a few hundred
/// frames, so a cycle empties it, and a flooded bus cannot hold the cycle; the rest wait for the
/// next cycle.
const MAX_FRAMES_PER_CYCLE: usize = 1024;

/// The `can` driver: its controller, the exchange of frames, its `on_error`, and the health of its
/// input and its output path.
pub(super) struct CanDriver {
    port: Box<dyn CanPort>,
    exchange: CanExchange,
    on_error: OnError,
    /// The start, from which the exchange's milliseconds count.
    started: Instant,
    /// Whether the controller is bus-off: from the error frame that says so until one that says it
    /// restarted or is error-active again, or until its state is no longer bus-off.
    bus_off: bool,
    /// The failed send of the last output handoff, if one failed.
    send_failure: Option<String>,
    /// The input path's health: a failed receive or a receive's timeout.
    input: IoDriverHealth,
    /// The output path's health: bus-off or a failed send.
    output: IoDriverHealth,
}

impl CanDriver {
    /// The driver of `params` on this computer: its interface found under `/sys/class/net`, up and
    /// at `bitrate`, and a raw socket opened on it.
    ///
    /// # Errors
    ///
    /// The refusal of `tx` and `rx`, and of an interface that this computer lacks, that is not a
    /// CAN interface, that is down or that runs at another bit rate, and the failure to read its
    /// bit rate or to open the socket, each after the driver's name.
    pub(super) fn start(params: CanParams) -> Result<Self, RuntimeError> {
        check_pins(&params)?;
        let interface = params.line.as_str();
        check_interface(Path::new("/sys"), interface)?;
        let link = CanInterface::open(interface)
            .map_err(|error| failed(format!("find the CAN interface {interface}: {error}")))?;
        let reported = link
            .bit_rate()
            .map_err(|error| failed(format!("read the bit rate of {interface}: {error}")))?;
        check_bitrate(interface, params.bitrate, reported)?;
        let receive: Vec<CanId> = params.receive.iter().map(|receive| receive.id).collect();
        let socket = open_socket(interface, &receive)
            .map_err(|error| failed(format!("open a raw socket on {interface}: {error}")))?;
        let port = SocketPort {
            socket,
            link,
            interface: params.line.clone(),
        };
        Ok(Self::new(params, Box::new(port)))
    }

    /// The driver of `params` over `port`, its exchange started now.
    fn new(params: CanParams, port: Box<dyn CanPort>) -> Self {
        let on_error = params.on_error;
        Self {
            port,
            exchange: CanExchange::new(params, 0),
            on_error,
            started: Instant::now(),
            bus_off: false,
            send_failure: None,
            input: IoDriverHealth::Ok,
            output: IoDriverHealth::Ok,
        }
    }

    /// The milliseconds since the driver started.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Takes what the controller received, then writes each receive's newest data to `inputs` at
    /// `now_ms`, this cycle's start. Bus-off, a failed receive and a receive's timeout are the
    /// problems of the handoff, in that order.
    fn exchange_inputs(&mut self, inputs: &mut [u8], now_ms: u64) -> Result<(), RuntimeError> {
        let failure = self.take_received(now_ms).err();
        let timeout = self.exchange.read_inputs(inputs, now_ms).err();
        let problem = match (failure, timeout) {
            (Some(failure), _) => Some(Problem::Transport(failure)),
            (None, Some(timeout)) => Some(Problem::Freshness(timeout)),
            (None, None) => None,
        };
        self.input = self.health_of(problem.as_ref());
        self.output = self.health_of(self.output_problem().as_ref());
        let bus_off = self.bus_off.then(|| Problem::Transport(BUS_OFF.to_owned()));
        self.outcome(bus_off.or(problem))
    }

    /// Takes up to `MAX_FRAMES_PER_CYCLE` of what the controller received at `now_ms`: a data frame
    /// goes to the exchange, and an error frame may show bus-off or its end. A bus-off from an
    /// earlier cycle that no error frame ended ends when the controller's state is no longer
    /// bus-off, as a controller may recover on its own without saying so; a bus-off that came in
    /// this cycle is reported in this cycle. The error is a failed receive or state read.
    fn take_received(&mut self, now_ms: u64) -> Result<(), String> {
        let was_off = self.bus_off;
        for _ in 0..MAX_FRAMES_PER_CYCLE {
            match self.port.receive()? {
                None => break,
                Some(Received::Data(frame)) => self.exchange.received(&frame, now_ms),
                Some(Received::Error { bits, data }) => {
                    if let Some(off) = bus_off_change(bits, &data) {
                        self.bus_off = off;
                    }
                }
                Some(Received::Remote) => {}
            }
        }
        if was_off
            && self.bus_off
            && let Some(off) = self.port.bus_off()?
        {
            self.bus_off = off;
        }
        Ok(())
    }

    /// Sends each transmit due at `now_ms` with its bytes of `outputs`. Bus-off and a failed send
    /// are the problems of the handoff, in that order.
    fn exchange_outputs(&mut self, outputs: &[u8], now_ms: u64) -> Result<(), RuntimeError> {
        let port = &mut self.port;
        let mut failure = None;
        self.exchange.send_due(outputs, now_ms, |frame| {
            if let Err(error) = port.send(&frame)
                && failure.is_none()
            {
                failure = Some(error);
            }
        });
        self.send_failure = failure;
        let problem = self.output_problem();
        self.output = self.health_of(problem.as_ref());
        self.outcome(problem)
    }

    /// The output path's problem: bus-off, otherwise the failed send of the last output handoff.
    fn output_problem(&self) -> Option<Problem> {
        if self.bus_off {
            Some(Problem::Transport(BUS_OFF.to_owned()))
        } else {
            self.send_failure.clone().map(Problem::Transport)
        }
    }

    /// The health of a path with `problem` under `on_error`: faulted under `fault`, degraded under
    /// `warn` and `ignore`, as for Modbus.
    fn health_of(&self, problem: Option<&Problem>) -> IoDriverHealth {
        let Some(problem) = problem else {
            return IoDriverHealth::Ok;
        };
        let error = SmolStr::new(problem.text());
        match self.on_error {
            OnError::Fault => IoDriverHealth::Faulted { error },
            OnError::Warn | OnError::Ignore => IoDriverHealth::Degraded { error },
        }
    }

    /// What `problem` makes of a handoff under `on_error`: an error under `fault`, while under
    /// `warn` and `ignore` the resource runs on.
    fn outcome(&self, problem: Option<Problem>) -> Result<(), RuntimeError> {
        match problem {
            Some(problem) if self.on_error == OnError::Fault => Err(problem.into_error()),
            _ => Ok(()),
        }
    }
}

impl IoDriver for CanDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let now_ms = self.now_ms();
        self.exchange_inputs(inputs, now_ms)
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let now_ms = self.now_ms();
        self.exchange_outputs(outputs, now_ms)
    }

    /// The more severe of the input and the output path's health, the output path's on a tie.
    fn health(&self) -> IoDriverHealth {
        if severity(&self.input) > severity(&self.output) {
            self.input.clone()
        } else {
            self.output.clone()
        }
    }

    /// The output path's health: a receive's timeout concerns received data alone, so it never
    /// leaves a safe-state handoff unconfirmed (specification 11, safe-state handoff
    /// confirmation), while bus-off does.
    fn output_health(&self) -> IoDriverHealth {
        self.output.clone()
    }
}

/// How severe a health is: healthy, degraded, faulted.
fn severity(health: &IoDriverHealth) -> u8 {
    match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    }
}

/// A problem of a handoff: its text, which the driver's health shows, and the kind of error that it
/// is under `on_error = "fault"`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Problem {
    /// The controller or the socket: bus-off, a failed receive or send.
    Transport(String),
    /// A receive whose frame did not come within its timeout.
    Freshness(String),
}

impl Problem {
    /// The text the driver's health shows.
    fn text(&self) -> &str {
        match self {
            Self::Transport(text) | Self::Freshness(text) => text.as_str(),
        }
    }

    /// The error under `on_error = "fault"`, after the driver's name.
    fn into_error(self) -> RuntimeError {
        match self {
            Self::Transport(text) => RuntimeError::IoTransport(format!("{NAME}: {text}").into()),
            Self::Freshness(text) => RuntimeError::IoFreshness(format!("{NAME}: {text}").into()),
        }
    }
}

/// A CAN controller as the driver uses it: what it received, without waiting, and what it sends.
trait CanPort: Send {
    /// The next thing the controller received, `None` when nothing waits.
    fn receive(&mut self) -> Result<Option<Received>, String>;

    /// Sends `frame`.
    fn send(&mut self, frame: &CanFrame) -> Result<(), String>;

    /// Whether the controller is bus-off now, as its driver reports its state; `None` for a
    /// controller that reports no state, as a virtual interface.
    fn bus_off(&mut self) -> Result<Option<bool>, String>;
}

/// What a controller received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Received {
    /// A data frame.
    Data(CanFrame),
    /// An error frame: its error class bits and its data.
    Error {
        /// The class bits, `CAN_ERR_*` (`linux/can/error.h`).
        bits: u32,
        /// The data, whose bytes detail the classes.
        data: [u8; 8],
    },
    /// A remote frame, which no receive reads.
    Remote,
}

/// What an error frame with the class `bits` and `data` says of the bus (`linux/can/error.h`):
/// `Some(true)` when the controller went bus-off, `Some(false)` when it restarted or is
/// error-active again, `None` for any other error.
fn bus_off_change(bits: u32, data: &[u8; 8]) -> Option<bool> {
    if bits & ERR_BUS_OFF != 0 {
        Some(true)
    } else if bits & ERR_RESTARTED != 0
        || (bits & ERR_CONTROLLER != 0 && data[1] & CONTROLLER_ACTIVE != 0)
    {
        Some(false)
    } else {
        None
    }
}

/// The driver's raw socket on its interface, and the interface's netlink link for its state.
struct SocketPort {
    socket: CanSocket,
    link: CanInterface,
    /// The interface, which the errors name.
    interface: SmolStr,
}

impl CanPort for SocketPort {
    fn receive(&mut self) -> Result<Option<Received>, String> {
        match self.socket.read_frame() {
            Ok(socketcan::CanFrame::Data(frame)) => Ok(Some(Received::Data(data_frame(&frame)))),
            Ok(socketcan::CanFrame::Remote(_)) => Ok(Some(Received::Remote)),
            Ok(socketcan::CanFrame::Error(frame)) => Ok(Some(Received::Error {
                bits: frame.error_bits(),
                data: eight_bytes(frame.data()).0,
            })),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(format!("receive on {}: {error}", self.interface)),
        }
    }

    fn send(&mut self, frame: &CanFrame) -> Result<(), String> {
        let id = frame.id.id;
        let Some(outgoing) = socket_frame(frame) else {
            return Err(format!("the identifier {id:#x} does not fit its frame"));
        };
        self.socket
            .write_frame(&outgoing)
            .map_err(|error| format!("send {id:#x} on {}: {error}", self.interface))
    }

    fn bus_off(&mut self) -> Result<Option<bool>, String> {
        self.link
            .state()
            .map(|state| state.map(|state| state == CanState::BusOff))
            .map_err(|error| format!("read the state of {}: {error}", self.interface))
    }
}

/// A raw socket on `interface` that does not wait to receive, that takes the data frames of
/// `receive` alone, and the error frames of bus-off and of its end. Past `FILTER_MAX` receives the
/// socket takes every data frame, and the exchange drops the others.
fn open_socket(interface: &str, receive: &[CanId]) -> io::Result<CanSocket> {
    let socket = CanSocket::open(interface)?;
    socket.set_nonblocking(true)?;
    socket.set_error_filter(ERR_CONTROLLER | ERR_BUS_OFF | ERR_RESTARTED)?;
    if receive.is_empty() {
        socket.set_filter_drop_all()?;
    } else if receive.len() <= FILTER_MAX {
        let filters: Vec<CanFilter> = receive
            .iter()
            .map(|id| {
                let (can_id, mask) = filter_words(*id);
                CanFilter::new(can_id, mask)
            })
            .collect();
        socket.set_filters(&filters)?;
    }
    Ok(socket)
}

/// The kernel filter, as `(can_id, can_mask)`, that passes the data frames of `id` alone: its 11
/// or 29 bits, the extended flag and the remote flag must all match (`CAN_RAW_FILTER`).
fn filter_words(id: CanId) -> (u32, u32) {
    if id.extended {
        (id.id | EFF_FLAG, EFF_MASK | EFF_FLAG | RTR_FLAG)
    } else {
        (id.id, SFF_MASK | EFF_FLAG | RTR_FLAG)
    }
}

/// The frame of a received SocketCAN data frame.
fn data_frame(frame: &CanDataFrame) -> CanFrame {
    let (data, len) = eight_bytes(frame.data());
    CanFrame {
        id: CanId {
            id: frame.raw_id(),
            extended: frame.is_extended(),
        },
        len,
        data,
    }
}

/// The SocketCAN data frame of `frame`; `None` for an identifier wider than its 11 or 29 bits.
fn socket_frame(frame: &CanFrame) -> Option<CanDataFrame> {
    let id = if frame.id.extended {
        Id::Extended(ExtendedId::new(frame.id.id)?)
    } else {
        Id::Standard(StandardId::new(u16::try_from(frame.id.id).ok()?)?)
    };
    let len = usize::from(frame.len).min(frame.data.len());
    CanDataFrame::new(id, &frame.data[..len])
}

/// The first 8 bytes of `bytes` with zeros after them, and how many there were.
fn eight_bytes(bytes: &[u8]) -> ([u8; 8], u8) {
    let mut data = [0; 8];
    let len = bytes.len().min(data.len());
    data[..len].copy_from_slice(&bytes[..len]);
    (data, u8::try_from(len).unwrap_or(8))
}

/// Refuses `tx` and `rx`: on Linux the interface's own controller has its pins (specification 11
/// §6.5).
fn check_pins(params: &CanParams) -> Result<(), RuntimeError> {
    for (key, pin) in [("tx", &params.tx), ("rx", &params.rx)] {
        if let Some(pin) = pin {
            return Err(refused(format!(
                "{key} '{pin}' is refused on Linux, where the interface {} has its own pins",
                params.line
            )));
        }
    }
    Ok(())
}

/// Refuses `interface` when it is not a network interface of this computer, is not a CAN
/// interface or is down, from its files under `<sysfs>/class/net`.
fn check_interface(sysfs: &Path, interface: &str) -> Result<(), RuntimeError> {
    if !is_interface_name(interface) {
        return Err(refused(format!(
            "line '{interface}' is not a SocketCAN interface such as can0"
        )));
    }
    let folder = sysfs.join(NETWORK_INTERFACES).join(interface);
    if !folder.is_dir() {
        return Err(refused(format!(
            "this computer has no CAN interface {interface}"
        )));
    }
    if read_attribute(&folder.join("type"))?.trim() != ARPHRD_CAN {
        return Err(refused(format!("{interface} is not a CAN interface")));
    }
    let path = folder.join("flags");
    let text = read_attribute(&path)?;
    let Some(flags) = text
        .trim()
        .strip_prefix("0x")
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
    else {
        return Err(failed(format!(
            "{} holds '{}', not the flags of an interface",
            path.display(),
            text.trim()
        )));
    };
    if flags & IFF_UP == 0 {
        return Err(refused(format!("the CAN interface {interface} is down")));
    }
    Ok(())
}

/// Refuses an interface that runs at another rate than `bitrate`, with both numbers
/// (specification 11 §6.5). A virtual interface, `vcan`, reports no rate and has none to differ.
fn check_bitrate(interface: &str, bitrate: u32, reported: Option<u32>) -> Result<(), RuntimeError> {
    match reported {
        Some(rate) if rate != bitrate => Err(refused(format!(
            "the CAN interface {interface} runs at {rate} bit/s, not the bitrate {bitrate} bit/s \
             of its parameters"
        ))),
        _ => Ok(()),
    }
}

/// Whether `name` can name a network interface: 1 to 15 bytes, neither `.` nor `..`, without `/`
/// or white space.
fn is_interface_name(name: &str) -> bool {
    (1..=INTERFACE_NAME_MAX).contains(&name.len())
        && !matches!(name, "." | "..")
        && !name.contains(|character: char| character == '/' || character.is_whitespace())
}

/// The content of the sysfs file at `path`.
fn read_attribute(path: &Path) -> Result<String, RuntimeError> {
    fs::read_to_string(path)
        .map_err(|error| failed(format!("read {} failed: {error}", path.display())))
}

/// The driver's refusal when it starts: its name and `reason`.
fn refused(reason: String) -> RuntimeError {
    RuntimeError::InvalidConfig(format!("{NAME}: {reason}").into())
}

/// A failure of the driver's interface or socket: its name and `text`.
fn failed(text: String) -> RuntimeError {
    RuntimeError::IoDriver(format!("{NAME}: {text}").into())
}

#[cfg(test)]
#[path = "can_tests.rs"]
mod tests;
