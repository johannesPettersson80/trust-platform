//! Modbus TCP I/O driver.

#![allow(missing_docs)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration as StdDuration;

use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverErrorPolicy, IoDriverHealth};

mod worker;

mod point_map;
use point_map::{
    decode_modbus_numeric, encode_modbus_numeric, read_image_bool, read_image_numeric,
    validate_modbus_point_maps, write_image_bool, write_image_numeric, ModbusInputPoint,
    ModbusOutputPoint, ModbusPointToml,
};
use worker::ModbusWorker;

#[derive(Debug, Clone)]
pub struct ModbusTcpConfig {
    pub address: SocketAddr,
    pub unit_id: u8,
    pub input_start: u16,
    pub output_start: u16,
    pub timeout: StdDuration,
    pub on_error: IoDriverErrorPolicy,
    input_function: ModbusInputFunction,
    output_function: ModbusOutputFunction,
    input_points: Vec<ModbusInputPoint>,
    output_points: Vec<ModbusOutputPoint>,
}

impl ModbusTcpConfig {
    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        let params: ModbusToml = value
            .clone()
            .try_into()
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.params: {err}").into()))?;
        let address = params.address.parse::<SocketAddr>().map_err(|err| {
            RuntimeError::InvalidConfig(format!("io.params.address: {err}").into())
        })?;
        let timeout = StdDuration::from_millis(params.timeout_ms.unwrap_or(500));
        let on_error = params
            .on_error
            .as_deref()
            .map(IoDriverErrorPolicy::parse)
            .transpose()?
            .unwrap_or(IoDriverErrorPolicy::Fault);
        let input_function = params
            .input_function
            .as_deref()
            .map(ModbusInputFunction::parse)
            .transpose()?
            .unwrap_or(ModbusInputFunction::InputRegisters);
        let output_function = params
            .output_function
            .as_deref()
            .map(ModbusOutputFunction::parse)
            .transpose()?
            .unwrap_or(ModbusOutputFunction::MultipleRegisters);
        let input_points = params
            .input_points
            .unwrap_or_default()
            .into_iter()
            .map(|point| ModbusInputPoint::from_toml(point, input_function))
            .collect::<Result<Vec<_>, _>>()?;
        let output_points = params
            .output_points
            .unwrap_or_default()
            .into_iter()
            .map(|point| ModbusOutputPoint::from_toml(point, output_function))
            .collect::<Result<Vec<_>, _>>()?;
        validate_modbus_point_maps(&input_points, &output_points)?;
        Ok(Self {
            address,
            unit_id: params.unit_id.unwrap_or(1),
            input_start: params.input_start.unwrap_or(0),
            output_start: params.output_start.unwrap_or(0),
            timeout,
            on_error,
            input_function,
            output_function,
            input_points,
            output_points,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ModbusToml {
    address: String,
    unit_id: Option<u8>,
    input_start: Option<u16>,
    output_start: Option<u16>,
    timeout_ms: Option<u64>,
    on_error: Option<String>,
    input_function: Option<String>,
    output_function: Option<String>,
    input_points: Option<Vec<ModbusPointToml>>,
    output_points: Option<Vec<ModbusPointToml>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModbusInputFunction {
    Coils,
    DiscreteInputs,
    HoldingRegisters,
    InputRegisters,
}

impl ModbusInputFunction {
    fn parse(value: &str) -> Result<Self, RuntimeError> {
        match normalize_function_name(value).as_str() {
            "read_coils" | "coil" | "coils" | "fc01" | "01" => Ok(Self::Coils),
            "read_discrete_inputs" | "discrete_inputs" | "discrete" | "fc02" | "02" => {
                Ok(Self::DiscreteInputs)
            }
            "read_holding_registers" | "holding_registers" | "holding" | "fc03" | "03" => {
                Ok(Self::HoldingRegisters)
            }
            "read_input_registers" | "input_registers" | "input" | "fc04" | "04" => {
                Ok(Self::InputRegisters)
            }
            _ => Err(RuntimeError::InvalidConfig(
                format!(
                    "io.params.input_function: unsupported Modbus function '{value}', expected \
                     read_coils, read_discrete_inputs, read_holding_registers, or \
                     read_input_registers"
                )
                .into(),
            )),
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Coils => 0x01,
            Self::DiscreteInputs => 0x02,
            Self::HoldingRegisters => 0x03,
            Self::InputRegisters => 0x04,
        }
    }

    fn is_bit_read(self) -> bool {
        matches!(self, Self::Coils | Self::DiscreteInputs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModbusOutputFunction {
    SingleCoil,
    SingleRegister,
    MultipleCoils,
    MultipleRegisters,
}

impl ModbusOutputFunction {
    fn parse(value: &str) -> Result<Self, RuntimeError> {
        match normalize_function_name(value).as_str() {
            "write_single_coil" | "single_coil" | "coil" | "fc05" | "05" => Ok(Self::SingleCoil),
            "write_single_register" | "single_register" | "register" | "fc06" | "06" => {
                Ok(Self::SingleRegister)
            }
            "write_multiple_coils" | "multiple_coils" | "coils" | "fc15" | "15" | "0f" => {
                Ok(Self::MultipleCoils)
            }
            "write_multiple_registers"
            | "multiple_registers"
            | "registers"
            | "fc16"
            | "16"
            | "10" => Ok(Self::MultipleRegisters),
            _ => Err(RuntimeError::InvalidConfig(
                format!(
                    "io.params.output_function: unsupported Modbus function '{value}', expected \
                     write_single_coil, write_single_register, write_multiple_coils, or \
                     write_multiple_registers"
                )
                .into(),
            )),
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::SingleCoil => 0x05,
            Self::SingleRegister => 0x06,
            Self::MultipleCoils => 0x0F,
            Self::MultipleRegisters => 0x10,
        }
    }
}

pub(super) fn normalize_function_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

#[derive(Debug)]
pub struct ModbusTcpDriver {
    address: SocketAddr,
    unit_id: u8,
    input_start: u16,
    output_start: u16,
    timeout: StdDuration,
    on_error: IoDriverErrorPolicy,
    input_function: ModbusInputFunction,
    output_function: ModbusOutputFunction,
    input_points: Vec<ModbusInputPoint>,
    output_points: Vec<ModbusOutputPoint>,
    transaction_id: u16,
    stream: Option<TcpStream>,
    health: IoDriverHealth,
    worker: Option<ModbusWorker>,
}

impl ModbusTcpDriver {
    pub fn new(config: ModbusTcpConfig) -> Self {
        let worker = Some(ModbusWorker::start(config.clone()));
        Self::from_config(config, worker)
    }

    fn new_sync_for_worker(config: ModbusTcpConfig) -> Self {
        Self::from_config(config, None)
    }

    fn from_config(config: ModbusTcpConfig, worker: Option<ModbusWorker>) -> Self {
        Self {
            address: config.address,
            unit_id: config.unit_id,
            input_start: config.input_start,
            output_start: config.output_start,
            timeout: config.timeout,
            on_error: config.on_error,
            input_function: config.input_function,
            output_function: config.output_function,
            input_points: config.input_points,
            output_points: config.output_points,
            transaction_id: 1,
            stream: None,
            health: IoDriverHealth::Ok,
            worker,
        }
    }

    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        let config = ModbusTcpConfig::from_params(value)?;
        Ok(Self::new(config))
    }

    fn ensure_connected(&mut self) -> Result<(), RuntimeError> {
        if self.stream.is_some() {
            return Ok(());
        }
        let stream = TcpStream::connect_timeout(&self.address, self.timeout).map_err(|err| {
            RuntimeError::IoTransport(format!("modbus tcp connect {}: {err}", self.address).into())
        })?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(self.timeout));
        let _ = stream.set_write_timeout(Some(self.timeout));
        self.stream = Some(stream);
        self.health = IoDriverHealth::Ok;
        Ok(())
    }

    fn next_transaction(&mut self) -> u16 {
        let current = self.transaction_id;
        self.transaction_id = self.transaction_id.wrapping_add(1);
        current
    }

    fn read_bits(
        &mut self,
        function: ModbusInputFunction,
        start: u16,
        qty: u16,
    ) -> Result<Vec<u8>, RuntimeError> {
        let pdu = [
            function.code(),
            (start >> 8) as u8,
            start as u8,
            (qty >> 8) as u8,
            qty as u8,
        ];
        let response = self.send_request(&pdu)?;
        ensure_modbus_response(&response, function.code(), 2)?;
        let byte_count = response[1] as usize;
        if response.len() < 2 + byte_count {
            return Err(RuntimeError::IoDriver("modbus response truncated".into()));
        }
        Ok(response[2..2 + byte_count].to_vec())
    }

    fn read_registers(
        &mut self,
        function: ModbusInputFunction,
        start: u16,
        qty: u16,
    ) -> Result<Vec<u8>, RuntimeError> {
        let pdu = [
            function.code(),
            (start >> 8) as u8,
            start as u8,
            (qty >> 8) as u8,
            qty as u8,
        ];
        let response = self.send_request(&pdu)?;
        ensure_modbus_response(&response, function.code(), 2)?;
        let byte_count = response[1] as usize;
        if response.len() < 2 + byte_count {
            return Err(RuntimeError::IoDriver("modbus response truncated".into()));
        }
        Ok(response[2..2 + byte_count].to_vec())
    }

    fn write_single_coil(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let value = if data.first().copied().unwrap_or(0) & 0x01 != 0 {
            0xFF00u16
        } else {
            0x0000u16
        };
        let pdu = [
            ModbusOutputFunction::SingleCoil.code(),
            (start >> 8) as u8,
            start as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        let response = self.send_request(&pdu)?;
        ensure_modbus_response(&response, ModbusOutputFunction::SingleCoil.code(), 5)
    }

    fn write_single_register(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let value = u16::from_be_bytes([
            data.first().copied().unwrap_or(0),
            data.get(1).copied().unwrap_or(0),
        ]);
        let pdu = [
            ModbusOutputFunction::SingleRegister.code(),
            (start >> 8) as u8,
            start as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        let response = self.send_request(&pdu)?;
        ensure_modbus_response(&response, ModbusOutputFunction::SingleRegister.code(), 5)
    }

    fn write_coils(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let qty = checked_bit_quantity(data.len())?;
        self.write_coils_range(start, qty, data)
    }

    fn write_coils_range(&mut self, start: u16, qty: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let byte_count = u8::try_from(data.len()).map_err(|_| {
            RuntimeError::IoDriver("modbus coil write exceeds single-request byte count".into())
        })?;
        let mut payload = Vec::with_capacity(6 + data.len());
        payload.push(ModbusOutputFunction::MultipleCoils.code());
        payload.push((start >> 8) as u8);
        payload.push(start as u8);
        payload.push((qty >> 8) as u8);
        payload.push(qty as u8);
        payload.push(byte_count);
        payload.extend_from_slice(data);
        let response = self.send_request(&payload)?;
        ensure_modbus_response(&response, ModbusOutputFunction::MultipleCoils.code(), 5)
    }

    fn write_registers(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let qty = data.len().div_ceil(2) as u16;
        let byte_count = (qty as usize) * 2;
        let mut payload = Vec::with_capacity(6 + byte_count);
        payload.push(ModbusOutputFunction::MultipleRegisters.code());
        payload.push((start >> 8) as u8);
        payload.push(start as u8);
        payload.push((qty >> 8) as u8);
        payload.push(qty as u8);
        payload.push(byte_count as u8);
        payload.extend(std::iter::repeat_n(0u8, byte_count));
        payload[6..6 + data.len()].copy_from_slice(data);
        let response = self.send_request(&payload)?;
        ensure_modbus_response(&response, ModbusOutputFunction::MultipleRegisters.code(), 5)
    }

    fn write_single_register_raw(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        if data.len() != 2 {
            return Err(RuntimeError::IoDriver(
                "modbus single-register point must encode exactly two bytes".into(),
            ));
        }
        let pdu = [
            ModbusOutputFunction::SingleRegister.code(),
            (start >> 8) as u8,
            start as u8,
            data[0],
            data[1],
        ];
        let response = self.send_request(&pdu)?;
        ensure_modbus_response(&response, ModbusOutputFunction::SingleRegister.code(), 5)
    }

    fn read_mapped_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        for point in self.input_points.clone() {
            if point.function.is_bit_read() {
                let data = self.read_bits(point.function, point.address, 1)?;
                let flag = data.first().copied().unwrap_or(0) & 0x01 != 0;
                write_image_bool(inputs, point.image_offset, point.image_bit, flag)?;
                continue;
            }

            let data = self.read_registers(
                point.function,
                point.address,
                point.data_type.register_count(),
            )?;
            let raw =
                decode_modbus_numeric(point.data_type, &data, point.byte_order, point.word_order)?;
            let scaled = raw * point.scale + point.offset;
            write_image_numeric(inputs, point.image_offset, point.data_type, scaled)?;
        }
        Ok(())
    }

    fn write_mapped_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        for point in self.output_points.clone() {
            if point.function.is_coil_write() {
                let flag = read_image_bool(outputs, point.image_offset, point.image_bit)?;
                let packed = [u8::from(flag)];
                match point.function {
                    ModbusOutputFunction::SingleCoil => {
                        self.write_single_coil(point.address, &packed)?;
                    }
                    ModbusOutputFunction::MultipleCoils => {
                        self.write_coils_range(point.address, 1, &packed)?;
                    }
                    ModbusOutputFunction::SingleRegister
                    | ModbusOutputFunction::MultipleRegisters => {
                        unreachable!("coil write branch only handles coil functions")
                    }
                }
                continue;
            }

            let engineered = read_image_numeric(outputs, point.image_offset, point.data_type)?;
            let raw = (engineered - point.offset) / point.scale;
            let wire =
                encode_modbus_numeric(point.data_type, raw, point.byte_order, point.word_order)?;
            match point.function {
                ModbusOutputFunction::SingleRegister => {
                    self.write_single_register_raw(point.address, &wire)?;
                }
                ModbusOutputFunction::MultipleRegisters => {
                    self.write_registers(point.address, &wire)?;
                }
                ModbusOutputFunction::SingleCoil | ModbusOutputFunction::MultipleCoils => {
                    unreachable!("register write branch only handles register functions")
                }
            }
        }
        Ok(())
    }

    fn send_request(&mut self, pdu: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        self.ensure_connected()?;
        let tx = self.next_transaction();
        let length = (pdu.len() + 1) as u16;
        let mut header = [0u8; 6];
        header[0..2].copy_from_slice(&tx.to_be_bytes());
        header[2..4].copy_from_slice(&0u16.to_be_bytes());
        header[4..6].copy_from_slice(&length.to_be_bytes());

        if let Some(stream) = self.stream.as_mut() {
            stream.write_all(&header).map_err(|err| {
                RuntimeError::IoTransport(format!("modbus write header: {err}").into())
            })?;
            stream.write_all(&[self.unit_id]).map_err(|err| {
                RuntimeError::IoTransport(format!("modbus write unit id: {err}").into())
            })?;
            stream.write_all(pdu).map_err(|err| {
                RuntimeError::IoTransport(format!("modbus write pdu: {err}").into())
            })?;
            stream
                .flush()
                .map_err(|err| RuntimeError::IoTransport(format!("modbus flush: {err}").into()))?;

            let mut resp_header = [0u8; 6];
            stream.read_exact(&mut resp_header).map_err(|err| {
                RuntimeError::IoTransport(format!("modbus read header: {err}").into())
            })?;
            let resp_tx = u16::from_be_bytes([resp_header[0], resp_header[1]]);
            if resp_tx != tx {
                return Err(RuntimeError::IoDriver(
                    format!("modbus transaction mismatch {resp_tx} != {tx}").into(),
                ));
            }
            let length = u16::from_be_bytes([resp_header[4], resp_header[5]]) as usize;
            let mut resp_body = vec![0u8; length];
            stream.read_exact(&mut resp_body).map_err(|err| {
                RuntimeError::IoTransport(format!("modbus read body: {err}").into())
            })?;
            if resp_body.is_empty() {
                return Err(RuntimeError::IoDriver("modbus response empty".into()));
            }
            Ok(resp_body[1..].to_vec())
        } else {
            Err(RuntimeError::IoTransport("modbus tcp not connected".into()))
        }
    }

    fn handle_error(&mut self, err: RuntimeError) -> Result<(), RuntimeError> {
        let message = SmolStr::new(err.to_string());
        match self.on_error {
            IoDriverErrorPolicy::Fault => {
                self.health = IoDriverHealth::Faulted {
                    error: message.clone(),
                };
                self.stream = None;
                Err(err)
            }
            IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => {
                self.health = IoDriverHealth::Degraded {
                    error: message.clone(),
                };
                self.stream = None;
                Ok(())
            }
        }
    }

    fn mark_ok(&mut self) {
        self.health = IoDriverHealth::Ok;
    }
}

fn ensure_modbus_response(
    response: &[u8],
    expected_function: u8,
    min_len: usize,
) -> Result<(), RuntimeError> {
    if response.is_empty() {
        return Err(RuntimeError::IoDriver("modbus response too short".into()));
    }
    if response[0] & 0x80 != 0 {
        let code = response.get(1).copied().unwrap_or(0);
        return Err(RuntimeError::IoAddress(
            format!("modbus exception code {code}").into(),
        ));
    }
    if response.len() < min_len {
        return Err(RuntimeError::IoDriver("modbus response too short".into()));
    }
    if response[0] != expected_function {
        return Err(RuntimeError::IoDriver(
            format!(
                "modbus response function 0x{:02x} did not match request 0x{:02x}",
                response[0], expected_function
            )
            .into(),
        ));
    }
    Ok(())
}

fn checked_bit_quantity(byte_len: usize) -> Result<u16, RuntimeError> {
    let bit_len = byte_len
        .checked_mul(8)
        .ok_or_else(|| RuntimeError::IoDriver("modbus bit quantity overflow".into()))?;
    u16::try_from(bit_len)
        .map_err(|_| RuntimeError::IoDriver("modbus bit quantity exceeds u16".into()))
}

impl IoDriver for ModbusTcpDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if let Some(worker) = &self.worker {
            return worker.read_inputs(inputs);
        }
        if inputs.is_empty() && self.input_points.is_empty() {
            return Ok(());
        }
        let result = if !self.input_points.is_empty() {
            self.read_mapped_inputs(inputs)
        } else if self.input_function.is_bit_read() {
            checked_bit_quantity(inputs.len())
                .and_then(|qty| self.read_bits(self.input_function, self.input_start, qty))
                .map(|data| {
                    let len = inputs.len().min(data.len());
                    inputs[..len].copy_from_slice(&data[..len]);
                })
        } else {
            let qty = inputs.len().div_ceil(2) as u16;
            self.read_registers(self.input_function, self.input_start, qty)
                .map(|data| {
                    let len = inputs.len().min(data.len());
                    inputs[..len].copy_from_slice(&data[..len]);
                })
        };
        match result {
            Ok(()) => {
                self.mark_ok();
                Ok(())
            }
            Err(err) => self.handle_error(err),
        }
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        if let Some(worker) = &self.worker {
            return worker.write_outputs(outputs);
        }
        if outputs.is_empty() && self.output_points.is_empty() {
            return Ok(());
        }
        let result = if !self.output_points.is_empty() {
            self.write_mapped_outputs(outputs)
        } else {
            match self.output_function {
                ModbusOutputFunction::SingleCoil => {
                    self.write_single_coil(self.output_start, outputs)
                }
                ModbusOutputFunction::SingleRegister => {
                    self.write_single_register(self.output_start, outputs)
                }
                ModbusOutputFunction::MultipleCoils => self.write_coils(self.output_start, outputs),
                ModbusOutputFunction::MultipleRegisters => {
                    self.write_registers(self.output_start, outputs)
                }
            }
        };
        match result {
            Ok(()) => {
                self.mark_ok();
                Ok(())
            }
            Err(err) => self.handle_error(err),
        }
    }

    fn health(&self) -> IoDriverHealth {
        if let Some(worker) = &self.worker {
            return worker.health();
        }
        self.health.clone()
    }
}
