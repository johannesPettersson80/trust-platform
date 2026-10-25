//! The requests of one input refresh or output handoff (specification 11 §6.5): what each request
//! reads or writes, the frame that `trust-modbus` writes for it, and the response that
//! `trust-modbus` checks before any of its data is taken. A raw image is exchanged in requests of
//! at most the function's quantity in address order, a point map with one request for each point
//! in its list's order.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use trust_modbus::tcp;
use trust_modbus::{Bits, Error as WireError, FunctionCode, Registers, Request, Response};

use super::params::{ModbusTcpAddress, ModbusTcpParams, OutputFunction};
use super::plan::RawPlan;
use super::point_map::{self, InputPoint, Scalar};
use super::Failure;

/// The packed value of one coil that a Write Multiple Coils request sets ON.
const COIL_ON: [u8; 1] = [1];

/// The packed value of one coil that a Write Multiple Coils request sets OFF.
const COIL_OFF: [u8; 1] = [0];

/// The requests of an input refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputPlan {
    /// The raw image, in requests of at most the function's quantity.
    Raw(RawPlan),
    /// One request for each of this many points, in `input_points` order.
    Points(usize),
}

impl InputPlan {
    /// How many requests the refresh sends.
    pub(super) fn requests(self) -> usize {
        match self {
            Self::Raw(plan) => plan.requests(),
            Self::Points(count) => count,
        }
    }
}

/// The data of an input refresh: the bytes of the raw image, or the value of each point so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InputData {
    /// The image bytes.
    Raw(Vec<u8>),
    /// The values of the points, in `input_points` order.
    Points(Vec<Scalar>),
}

/// An input refresh: the request it serves, its requests, the next of them and its data so far.
#[derive(Debug, Clone)]
pub(super) struct InputRun {
    /// The number of the scan's request it serves.
    pub(super) seq: u64,
    /// Its requests.
    pub(super) plan: InputPlan,
    /// The next request.
    pub(super) next: usize,
    /// What the responses gave so far.
    pub(super) data: InputData,
}

impl InputRun {
    /// The refresh of request `seq` with `plan`, for an image of `len` bytes.
    pub(super) fn new(seq: u64, plan: InputPlan, len: usize) -> Self {
        let data = match plan {
            InputPlan::Raw(_) => InputData::Raw(vec![0; len]),
            InputPlan::Points(count) => InputData::Points(Vec::with_capacity(count)),
        };
        Self {
            seq,
            plan,
            next: 0,
            data,
        }
    }
}

/// What an output handoff writes.
#[derive(Debug, Clone)]
pub(super) enum OutputData {
    /// The raw image's bytes, with a zero byte after an odd number of them for registers.
    Raw {
        /// The requests.
        plan: RawPlan,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// The wire value of each point, in `output_points` order.
    Points(Vec<Scalar>),
}

impl OutputData {
    /// How many requests the handoff sends.
    pub(super) fn requests(&self) -> usize {
        match self {
            Self::Raw { plan, .. } => plan.requests(),
            Self::Points(values) => values.len(),
        }
    }
}

/// An output handoff on its way: the handoff it is, what it writes and its next request.
#[derive(Debug, Clone)]
pub(super) struct OutputRun {
    /// The number of the scan's handoff.
    pub(super) seq: u64,
    /// What it writes.
    pub(super) data: OutputData,
    /// The next request.
    pub(super) next: usize,
}

/// What a response does to its run.
#[derive(Debug)]
pub(super) enum Step {
    /// The run has more requests to send.
    More,
    /// The run completed.
    Done,
    /// The run failed.
    Failed(Failure),
}

/// The requests of an input refresh of an image of `len` bytes: the raw image's, or one for each
/// point, which must all lie in the image.
pub(super) fn input_plan(params: &ModbusTcpParams, len: usize) -> Result<InputPlan, Failure> {
    if params.input_points.is_empty() {
        RawPlan::new(
            params.input_function.code(),
            params.input_start,
            len,
            "input",
        )
        .map(InputPlan::Raw)
        .map_err(Failure::data)
    } else {
        point_map::fits(&params.input_points, len)?;
        Ok(InputPlan::Points(params.input_points.len()))
    }
}

/// What an output handoff of `image` writes; `None` for an empty raw image, which writes nothing.
pub(super) fn output_data(
    params: &ModbusTcpParams,
    image: &[u8],
) -> Result<Option<OutputData>, Failure> {
    if params.output_points.is_empty() {
        let function = params.output_function.code();
        let plan = RawPlan::new(function, params.output_start, image.len(), "output")
            .map_err(Failure::data)?;
        if plan.requests() == 0 {
            return Ok(None);
        }
        let mut bytes = image.to_vec();
        if !params.output_function.writes_bits() && !bytes.len().is_multiple_of(2) {
            // The last register's low byte, which the image does not have (specification 11 §6.5:
            // `ceil(bytes / 2)` registers).
            bytes.push(0);
        }
        return Ok(Some(OutputData::Raw { plan, bytes }));
    }
    params
        .output_points
        .iter()
        .map(|point| point_map::encode_output(point, image))
        .collect::<Result<Vec<_>, _>>()
        .map(|values| Some(OutputData::Points(values)))
}

/// Read request `index` of an input refresh with `plan`, and its address; `None` past the plan.
pub(super) fn read_request(
    params: &ModbusTcpParams,
    plan: InputPlan,
    index: usize,
) -> Option<(Request<'static>, u16)> {
    let (function, address, quantity) = match plan {
        InputPlan::Raw(raw) => {
            let (address, quantity) = raw.request(index)?;
            (raw.function, address, quantity)
        }
        InputPlan::Points(_) => {
            let point = params.input_points.get(index)?;
            let quantity = point.data_type.registers().max(1);
            (point.function.code(), point.address, quantity)
        }
    };
    let request = match function {
        FunctionCode::ReadCoils => Request::ReadCoils { address, quantity },
        FunctionCode::ReadDiscreteInputs => Request::ReadDiscreteInputs { address, quantity },
        FunctionCode::ReadHoldingRegisters => Request::ReadHoldingRegisters { address, quantity },
        FunctionCode::ReadInputRegisters => Request::ReadInputRegisters { address, quantity },
        _ => return None,
    };
    Some((request, address))
}

/// Write request `index` of an output handoff that writes `data`, and its address.
pub(super) fn write_request<'a>(
    params: &ModbusTcpParams,
    data: &'a OutputData,
    index: usize,
) -> Result<(Request<'a>, u16), Failure> {
    match data {
        OutputData::Raw { plan, bytes } => {
            let (address, quantity) = plan.request(index).ok_or_else(unplanned)?;
            let part = bytes.get(plan.bytes(index)).ok_or_else(unplanned)?;
            let request = match plan.function {
                FunctionCode::WriteSingleCoil => Request::WriteSingleCoil {
                    address,
                    value: part.first().is_some_and(|byte| byte & 1 != 0),
                },
                FunctionCode::WriteSingleRegister => {
                    let [high, low] = *part else {
                        return Err(unplanned());
                    };
                    Request::WriteSingleRegister {
                        address,
                        value: u16::from_be_bytes([high, low]),
                    }
                }
                FunctionCode::WriteMultipleCoils => Request::WriteMultipleCoils {
                    address,
                    values: Bits::new(quantity, part).map_err(refused_request)?,
                },
                FunctionCode::WriteMultipleRegisters => Request::WriteMultipleRegisters {
                    address,
                    values: Registers::new(part).map_err(refused_request)?,
                },
                _ => return Err(unplanned()),
            };
            Ok((request, address))
        }
        OutputData::Points(values) => {
            let point = params.output_points.get(index).ok_or_else(unplanned)?;
            let value = values.get(index).ok_or_else(unplanned)?;
            let address = point.address;
            let request = match (point.function, value) {
                (OutputFunction::SingleCoil, Scalar::Bit(on)) => Request::WriteSingleCoil {
                    address,
                    value: *on,
                },
                (OutputFunction::MultipleCoils, Scalar::Bit(on)) => {
                    let packed: &'static [u8] = if *on { &COIL_ON } else { &COIL_OFF };
                    Request::WriteMultipleCoils {
                        address,
                        values: Bits::new(1, packed).map_err(refused_request)?,
                    }
                }
                (OutputFunction::SingleRegister, Scalar::Bytes { bytes, len: 2 }) => {
                    let [high, low, ..] = *bytes;
                    Request::WriteSingleRegister {
                        address,
                        value: u16::from_be_bytes([high, low]),
                    }
                }
                (OutputFunction::MultipleRegisters, Scalar::Bytes { bytes, len }) => {
                    let wire = bytes.get(..*len).ok_or_else(unplanned)?;
                    Request::WriteMultipleRegisters {
                        address,
                        values: Registers::new(wire).map_err(refused_request)?,
                    }
                }
                _ => return Err(unplanned()),
            };
            Ok((request, address))
        }
    }
}

/// Writes `request` as a Modbus TCP frame into `frame`, with `trust-modbus`, and gives its length.
pub(super) fn encode(
    transaction_id: u16,
    unit_id: u8,
    request: &Request<'_>,
    frame: &mut [u8],
) -> Result<usize, Failure> {
    tcp::encode_request(transaction_id, unit_id, request, frame).map_err(refused_request)
}

/// Takes the response `frame` to the request of transaction `transaction_id` of an input refresh,
/// checked with `trust-modbus` before any of its data is taken.
pub(super) fn take_read(
    params: &ModbusTcpParams,
    run: &mut InputRun,
    transaction_id: u16,
    frame: &[u8],
) -> Step {
    let Some((request, address)) = read_request(params, run.plan, run.next) else {
        return Step::Failed(unplanned());
    };
    let response = match tcp::decode_response(transaction_id, &request, frame) {
        Ok(response) => response,
        Err(error) => return Step::Failed(refusal(params.address, address, error)),
    };
    let index = run.next;
    let taken = match (run.plan, &mut run.data, response) {
        (InputPlan::Raw(plan), InputData::Raw(bytes), Response::Bits(bits)) => {
            copy_into(bytes, plan.bytes(index).start, bits.packed());
            Ok(())
        }
        (InputPlan::Raw(plan), InputData::Raw(bytes), Response::Registers(registers)) => {
            copy_into(bytes, plan.bytes(index).start, registers.as_bytes());
            Ok(())
        }
        (InputPlan::Points(_), InputData::Points(values), response) => {
            point_value(params, index, response).map(|value| values.push(value))
        }
        _ => Err(unplanned()),
    };
    match taken {
        Ok(()) => {
            run.next = index + 1;
            if run.next >= run.plan.requests() {
                Step::Done
            } else {
                Step::More
            }
        }
        Err(failure) => Step::Failed(failure),
    }
}

/// Takes the response `frame` to the request of transaction `transaction_id` of an output handoff,
/// checked with `trust-modbus`: the echo of the request's address and its value or quantity.
pub(super) fn take_write(
    params: &ModbusTcpParams,
    run: &mut OutputRun,
    transaction_id: u16,
    frame: &[u8],
) -> Step {
    let checked = match write_request(params, &run.data, run.next) {
        Ok((request, address)) => match tcp::decode_response(transaction_id, &request, frame) {
            Ok(Response::Written) => Ok(()),
            Ok(Response::Bits(_) | Response::Registers(_)) => Err(unplanned()),
            Err(error) => Err(refusal(params.address, address, error)),
        },
        Err(failure) => Err(failure),
    };
    match checked {
        Ok(()) => {
            run.next += 1;
            if run.next >= run.data.requests() {
                Step::Done
            } else {
                Step::More
            }
        }
        Err(failure) => Step::Failed(failure),
    }
}

/// The value of input point `index` from its response.
fn point_value(
    params: &ModbusTcpParams,
    index: usize,
    response: Response<'_>,
) -> Result<Scalar, Failure> {
    let point = params.input_points.get(index).ok_or_else(unplanned)?;
    match response {
        Response::Bits(bits) => bits.get(0).map(Scalar::Bit).ok_or_else(unplanned),
        Response::Registers(registers) => point_map::decode_input(point, registers.as_bytes()),
        Response::Written => Err(unplanned()),
    }
}

/// Copies `source` into `target` from its byte `first`, as far as `target` reaches: the last
/// register of an image of an odd length gives its high byte only.
fn copy_into(target: &mut [u8], first: usize, source: &[u8]) {
    let end = first.saturating_add(source.len()).min(target.len());
    let start = first.min(end);
    if let (Some(part), Some(data)) = (target.get_mut(start..end), source.get(..end - start)) {
        part.copy_from_slice(data);
    }
}

/// Copies `snapshot` into `image`: the raw bytes, or each point's bytes or bit after every point
/// was checked to lie in the image, so that a failure changes no byte.
pub(super) fn apply(
    snapshot: &InputData,
    points: &[InputPoint],
    image: &mut [u8],
) -> Result<(), Failure> {
    match snapshot {
        InputData::Raw(bytes) => {
            copy_into(image, 0, bytes);
            Ok(())
        }
        InputData::Points(values) => {
            point_map::fits(points, image.len())?;
            for (point, value) in points.iter().zip(values) {
                point_map::write_input(point, *value, image);
            }
            Ok(())
        }
    }
}

/// The failure of a response to the request at `address` that `trust-modbus` refused, from the
/// server at `server`: an exception response is the typed exception, with its code and the name
/// that V1.1b3 §7 gives it; any other refusal is a transport failure.
fn refusal(server: ModbusTcpAddress, address: u16, error: WireError) -> Failure {
    match error {
        WireError::Exception { function, code } => Failure::exception(format!(
            "modbus exception code {} ({}): {function} at address {address}",
            code.code(),
            code.name().unwrap_or("not listed in V1.1b3"),
        )),
        other => Failure::transport(format!("modbus tcp {server}: refused response: {other}")),
    }
}

/// The failure of a request that `trust-modbus` refused to write; the plan keeps every request
/// within the limits, so this does not happen.
fn refused_request(error: WireError) -> Failure {
    Failure::data(format!("modbus request refused: {error}"))
}

/// The failure of a request or a response outside the plan of its run, which does not happen.
pub(super) fn unplanned() -> Failure {
    Failure::data("modbus request outside the driver's plan")
}
