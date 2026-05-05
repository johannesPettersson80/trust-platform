//! Modbus TCP I/O driver.

#![allow(missing_docs)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration as StdDuration;

use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverErrorPolicy, IoDriverHealth};

#[derive(Debug, Clone)]
pub struct ModbusTcpConfig {
    pub address: SocketAddr,
    pub unit_id: u8,
    pub input_start: u16,
    pub output_start: u16,
    pub timeout: StdDuration,
    pub on_error: IoDriverErrorPolicy,
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
        Ok(Self {
            address,
            unit_id: params.unit_id.unwrap_or(1),
            input_start: params.input_start.unwrap_or(0),
            output_start: params.output_start.unwrap_or(0),
            timeout,
            on_error,
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
}

#[derive(Debug)]
pub struct ModbusTcpDriver {
    address: SocketAddr,
    unit_id: u8,
    input_start: u16,
    output_start: u16,
    timeout: StdDuration,
    on_error: IoDriverErrorPolicy,
    transaction_id: u16,
    stream: Option<TcpStream>,
    health: IoDriverHealth,
}

impl ModbusTcpDriver {
    pub fn new(config: ModbusTcpConfig) -> Self {
        Self {
            address: config.address,
            unit_id: config.unit_id,
            input_start: config.input_start,
            output_start: config.output_start,
            timeout: config.timeout,
            on_error: config.on_error,
            transaction_id: 1,
            stream: None,
            health: IoDriverHealth::Ok,
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

    fn read_registers(&mut self, start: u16, qty: u16) -> Result<Vec<u8>, RuntimeError> {
        let pdu = [
            0x04,
            (start >> 8) as u8,
            start as u8,
            (qty >> 8) as u8,
            qty as u8,
        ];
        let response = self.send_request(&pdu)?;
        if response.len() < 2 {
            return Err(RuntimeError::IoDriver("modbus response too short".into()));
        }
        if response[0] & 0x80 != 0 {
            let code = response.get(1).copied().unwrap_or(0);
            return Err(RuntimeError::IoAddress(
                format!("modbus exception code {code}").into(),
            ));
        }
        let byte_count = response[1] as usize;
        if response.len() < 2 + byte_count {
            return Err(RuntimeError::IoDriver("modbus response truncated".into()));
        }
        Ok(response[2..2 + byte_count].to_vec())
    }

    fn write_registers(&mut self, start: u16, data: &[u8]) -> Result<(), RuntimeError> {
        let qty = data.len().div_ceil(2) as u16;
        let byte_count = (qty as usize) * 2;
        let mut payload = Vec::with_capacity(6 + byte_count);
        payload.push(0x10);
        payload.push((start >> 8) as u8);
        payload.push(start as u8);
        payload.push((qty >> 8) as u8);
        payload.push(qty as u8);
        payload.push(byte_count as u8);
        payload.extend(std::iter::repeat_n(0u8, byte_count));
        payload[6..6 + data.len()].copy_from_slice(data);
        let response = self.send_request(&payload)?;
        if response.len() < 5 {
            return Err(RuntimeError::IoDriver("modbus response too short".into()));
        }
        if response[0] & 0x80 != 0 {
            let code = response.get(1).copied().unwrap_or(0);
            return Err(RuntimeError::IoAddress(
                format!("modbus exception code {code}").into(),
            ));
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
        if matches!(self.on_error, IoDriverErrorPolicy::Fault) {
            self.health = IoDriverHealth::Faulted {
                error: message.clone(),
            };
            self.stream = None;
            return Err(err);
        }
        self.health = IoDriverHealth::Degraded {
            error: message.clone(),
        };
        self.stream = None;
        Err(err)
    }

    fn mark_ok(&mut self) {
        self.health = IoDriverHealth::Ok;
    }
}

impl IoDriver for ModbusTcpDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if inputs.is_empty() {
            return Ok(());
        }
        let qty = inputs.len().div_ceil(2) as u16;
        match self.read_registers(self.input_start, qty) {
            Ok(data) => {
                let len = inputs.len().min(data.len());
                inputs[..len].copy_from_slice(&data[..len]);
                self.mark_ok();
                Ok(())
            }
            Err(err) => self.handle_error(err),
        }
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        if outputs.is_empty() {
            return Ok(());
        }
        match self.write_registers(self.output_start, outputs) {
            Ok(()) => {
                self.mark_ok();
                Ok(())
            }
            Err(err) => self.handle_error(err),
        }
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
