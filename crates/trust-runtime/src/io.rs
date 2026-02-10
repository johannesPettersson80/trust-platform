//! Direct I/O mapping and images.

#![allow(missing_docs)]

use smol_str::SmolStr;

mod modbus;
pub use modbus::ModbusTcpDriver;
mod mqtt;
pub use mqtt::MqttIoDriver;
mod gpio;
mod loopback;
mod registry;
pub use gpio::GpioDriver;
pub use loopback::LoopbackIoDriver;
pub use registry::IoDriverRegistry;

use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::memory::VariableStorage;
use crate::value::Value;
use crate::value::ValueRef;
use trust_hir::TypeId;

/// I/O driver interface for process image exchange.
pub trait IoDriver: Send {
    /// Read hardware or simulated inputs into the input image.
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError>;

    /// Write the output image to hardware or a simulator.
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError>;

    /// Report the current driver health.
    fn health(&self) -> IoDriverHealth {
        IoDriverHealth::Ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoDriverHealth {
    Ok,
    Degraded { error: SmolStr },
    Faulted { error: SmolStr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoDriverErrorPolicy {
    Fault,
    Warn,
    Ignore,
}

impl IoDriverErrorPolicy {
    pub fn parse(value: &str) -> Result<Self, RuntimeError> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "fault" => Ok(Self::Fault),
            "warn" | "warning" => Ok(Self::Warn),
            "ignore" => Ok(Self::Ignore),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid io.on_error '{value}' (expected fault/warn/ignore)").into(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IoDriverStatus {
    pub name: SmolStr,
    pub health: IoDriverHealth,
}

/// Default simulated I/O driver (no-op).
#[derive(Debug, Default)]
pub struct SimulatedIoDriver;

impl IoDriver for SimulatedIoDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoSize {
    Bit,
    Byte,
    Word,
    DWord,
    LWord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoAddress {
    pub area: IoArea,
    pub size: IoSize,
    pub byte: u32,
    pub bit: u8,
    pub path: Vec<u32>,
    pub wildcard: bool,
}

impl IoAddress {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        let trimmed = text.trim();
        if !trimmed.starts_with('%') {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        let mut chars = trimmed[1..].chars();
        let area = match chars.next() {
            Some('I') => IoArea::Input,
            Some('Q') => IoArea::Output,
            Some('M') => IoArea::Memory,
            _ => return Err(RuntimeError::InvalidIoAddress(trimmed.into())),
        };
        let rest: String = chars.collect();
        if rest.trim().is_empty() {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }

        let mut rest_chars = rest.chars();
        let first = rest_chars
            .next()
            .ok_or_else(|| RuntimeError::InvalidIoAddress(trimmed.into()))?;
        let (size, rest) = match first {
            'X' => (IoSize::Bit, rest_chars.as_str()),
            'B' => (IoSize::Byte, rest_chars.as_str()),
            'W' => (IoSize::Word, rest_chars.as_str()),
            'D' => (IoSize::DWord, rest_chars.as_str()),
            'L' => (IoSize::LWord, rest_chars.as_str()),
            '*' => {
                return Ok(Self {
                    area,
                    size: IoSize::Bit,
                    byte: 0,
                    bit: 0,
                    path: Vec::new(),
                    wildcard: true,
                })
            }
            ch if ch.is_ascii_digit() => (IoSize::Bit, rest.as_str()),
            _ => return Err(RuntimeError::InvalidIoAddress(trimmed.into())),
        };

        if rest.trim() == "*" {
            return Ok(Self {
                area,
                size,
                byte: 0,
                bit: 0,
                path: Vec::new(),
                wildcard: true,
            });
        }

        let mut path: Vec<u32> = Vec::new();
        let mut bit = 0u8;
        let parts: Vec<&str> = rest.split('.').collect();
        if parts.is_empty() {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        if matches!(size, IoSize::Bit) && parts.len() >= 2 {
            for part in &parts[..parts.len() - 1] {
                path.push(parse_u32(Some(part), trimmed)?);
            }
            let bit_part = parts
                .last()
                .copied()
                .ok_or_else(|| RuntimeError::InvalidIoAddress(trimmed.into()))?;
            bit = parse_u8(bit_part, trimmed)?;
            if bit > 7 {
                return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
            }
        } else {
            for part in &parts {
                path.push(parse_u32(Some(part), trimmed)?);
            }
        }
        if path.is_empty() {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        let byte = path[0];
        Ok(Self {
            area,
            size,
            byte,
            bit,
            path,
            wildcard: false,
        })
    }
}

#[derive(Debug, Clone)]
pub enum IoTarget {
    Name(SmolStr),
    Reference(ValueRef),
}

#[derive(Debug, Clone)]
pub struct IoBinding {
    pub target: IoTarget,
    pub address: IoAddress,
    pub value_type: Option<TypeId>,
    pub display_name: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub enum IoSnapshotValue {
    Value(Value),
    Error(String),
    Unresolved,
}

#[derive(Debug, Clone)]
pub struct IoSnapshotEntry {
    pub name: Option<SmolStr>,
    pub address: IoAddress,
    pub value: IoSnapshotValue,
}

#[derive(Debug, Clone, Default)]
pub struct IoSnapshot {
    pub inputs: Vec<IoSnapshotEntry>,
    pub outputs: Vec<IoSnapshotEntry>,
    pub memory: Vec<IoSnapshotEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct IoSafeState {
    pub outputs: Vec<(IoAddress, Value)>,
}

impl IoSafeState {
    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    pub fn apply(&self, io: &mut IoInterface) -> Result<(), RuntimeError> {
        for (address, value) in &self.outputs {
            io.write(address, value.clone())?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct IoInterface {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
    bindings: Vec<IoBinding>,
    hierarchical: std::collections::HashMap<IoAddressKey, Value>,
}

impl IoInterface {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn bindings(&self) -> &[IoBinding] {
        &self.bindings
    }

    /// Resize the process image buffers.
    pub fn resize(&mut self, inputs: usize, outputs: usize, memory: usize) {
        self.inputs.resize(inputs, 0);
        self.outputs.resize(outputs, 0);
        self.memory.resize(memory, 0);
    }

    /// Access the raw input image.
    #[must_use]
    pub fn inputs(&self) -> &[u8] {
        &self.inputs
    }

    /// Mutate the raw input image.
    pub fn inputs_mut(&mut self) -> &mut [u8] {
        &mut self.inputs
    }

    /// Access the raw output image.
    #[must_use]
    pub fn outputs(&self) -> &[u8] {
        &self.outputs
    }

    /// Mutate the raw output image.
    pub fn outputs_mut(&mut self) -> &mut [u8] {
        &mut self.outputs
    }

    /// Access the raw memory image.
    #[must_use]
    pub fn memory(&self) -> &[u8] {
        &self.memory
    }

    /// Mutate the raw memory image.
    pub fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    #[must_use]
    pub fn snapshot(&self) -> IoSnapshot {
        let mut snapshot = IoSnapshot::default();
        for binding in &self.bindings {
            let name = binding
                .display_name
                .clone()
                .or_else(|| match &binding.target {
                    IoTarget::Name(name) => Some(name.clone()),
                    IoTarget::Reference(_) => None,
                });
            let value = if binding.address.wildcard {
                IoSnapshotValue::Unresolved
            } else {
                match self.read(&binding.address) {
                    Ok(value) => IoSnapshotValue::Value(value),
                    Err(err) => IoSnapshotValue::Error(err.to_string()),
                }
            };
            let entry = IoSnapshotEntry {
                name,
                address: binding.address.clone(),
                value,
            };
            match binding.address.area {
                IoArea::Input => snapshot.inputs.push(entry),
                IoArea::Output => snapshot.outputs.push(entry),
                IoArea::Memory => snapshot.memory.push(entry),
            }
        }
        snapshot
    }

    pub fn bind(&mut self, name: impl Into<SmolStr>, address: IoAddress) {
        let name = name.into();
        self.bindings.push(IoBinding {
            target: IoTarget::Name(name.clone()),
            address,
            value_type: None,
            display_name: Some(name),
        });
    }

    pub fn bind_ref(&mut self, reference: ValueRef, address: IoAddress) {
        self.bindings.push(IoBinding {
            target: IoTarget::Reference(reference),
            address,
            value_type: None,
            display_name: None,
        });
    }

    pub fn bind_typed(&mut self, name: impl Into<SmolStr>, address: IoAddress, value_type: TypeId) {
        let name = name.into();
        self.bindings.push(IoBinding {
            target: IoTarget::Name(name.clone()),
            address,
            value_type: Some(value_type),
            display_name: Some(name),
        });
    }

    pub fn bind_ref_typed(&mut self, reference: ValueRef, address: IoAddress, value_type: TypeId) {
        self.bindings.push(IoBinding {
            target: IoTarget::Reference(reference),
            address,
            value_type: Some(value_type),
            display_name: None,
        });
    }

    pub fn bind_ref_named_typed(
        &mut self,
        reference: ValueRef,
        address: IoAddress,
        value_type: TypeId,
        name: impl Into<SmolStr>,
    ) {
        self.bindings.push(IoBinding {
            target: IoTarget::Reference(reference),
            address,
            value_type: Some(value_type),
            display_name: Some(name.into()),
        });
    }

    pub fn read_inputs(&self, storage: &mut VariableStorage) -> Result<(), RuntimeError> {
        for binding in &self.bindings {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            let value = self.read(&binding.address)?;
            let value = if let Some(value_type) = binding.value_type {
                coerce_from_io(value, value_type)?
            } else {
                value
            };
            match &binding.target {
                IoTarget::Name(name) => storage.set_global(name.clone(), value),
                IoTarget::Reference(reference) => {
                    if !storage.write_by_ref(reference.clone(), value) {
                        return Err(RuntimeError::NullReference);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn write_outputs(&mut self, storage: &VariableStorage) -> Result<(), RuntimeError> {
        let bindings = self.bindings.clone();
        for binding in bindings {
            if !matches!(binding.address.area, IoArea::Output | IoArea::Memory) {
                continue;
            }
            let value = match &binding.target {
                IoTarget::Name(name) => storage
                    .get_global(name.as_ref())
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?,
                IoTarget::Reference(reference) => storage
                    .read_by_ref(reference.clone())
                    .ok_or(RuntimeError::NullReference)?,
            };
            let value = if let Some(value_type) = binding.value_type {
                coerce_to_io(value.clone(), value_type, binding.address.size)?
            } else {
                value.clone()
            };
            self.write(&binding.address, value)?;
        }
        Ok(())
    }

    pub fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        if address.wildcard {
            return Err(RuntimeError::InvalidIoAddress(
                format!("%?* for {:?}", address.area).into(),
            ));
        }
        if address.path.len() > 1 {
            let key = IoAddressKey::from(address);
            return self.hierarchical.get(&key).cloned().ok_or_else(|| {
                RuntimeError::InvalidIoAddress(format!("hier {:?}", address.path).into())
            });
        }
        let buffer = self.area(address.area);
        match address.size {
            IoSize::Bit => {
                let byte = buffer.get(address.byte as usize).copied().unwrap_or(0);
                let bit = (byte >> address.bit) & 1;
                Ok(Value::Bool(bit == 1))
            }
            IoSize::Byte => Ok(Value::Byte(
                buffer.get(address.byte as usize).copied().unwrap_or(0),
            )),
            IoSize::Word => {
                let lo = buffer.get(address.byte as usize).copied().unwrap_or(0);
                let hi = buffer.get(address.byte as usize + 1).copied().unwrap_or(0);
                Ok(Value::Word(u16::from_le_bytes([lo, hi])))
            }
            IoSize::DWord => {
                let mut bytes = [0u8; 4];
                for (idx, byte) in bytes.iter_mut().enumerate() {
                    *byte = buffer
                        .get(address.byte as usize + idx)
                        .copied()
                        .unwrap_or(0);
                }
                Ok(Value::DWord(u32::from_le_bytes(bytes)))
            }
            IoSize::LWord => {
                let mut bytes = [0u8; 8];
                for (idx, byte) in bytes.iter_mut().enumerate() {
                    *byte = buffer
                        .get(address.byte as usize + idx)
                        .copied()
                        .unwrap_or(0);
                }
                Ok(Value::LWord(u64::from_le_bytes(bytes)))
            }
        }
    }

    pub fn write(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        if address.wildcard {
            return Err(RuntimeError::InvalidIoAddress(
                format!("%?* for {:?}", address.area).into(),
            ));
        }
        if address.path.len() > 1 {
            let key = IoAddressKey::from(address);
            self.hierarchical.insert(key, value);
            return Ok(());
        }
        let buffer = self.area_mut(address.area);
        match address.size {
            IoSize::Bit => match value {
                Value::Bool(flag) => {
                    ensure_len(buffer, address.byte as usize);
                    let byte = &mut buffer[address.byte as usize];
                    if flag {
                        *byte |= 1 << address.bit;
                    } else {
                        *byte &= !(1 << address.bit);
                    }
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::Byte => match value {
                Value::Byte(byte) => {
                    ensure_len(buffer, address.byte as usize);
                    buffer[address.byte as usize] = byte;
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::Word => match value {
                Value::Word(word) => {
                    ensure_len(buffer, address.byte as usize + 1);
                    let [lo, hi] = word.to_le_bytes();
                    buffer[address.byte as usize] = lo;
                    buffer[address.byte as usize + 1] = hi;
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::DWord => match value {
                Value::DWord(word) => {
                    ensure_len(buffer, address.byte as usize + 3);
                    let bytes = word.to_le_bytes();
                    for (idx, byte) in bytes.iter().enumerate() {
                        buffer[address.byte as usize + idx] = *byte;
                    }
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::LWord => match value {
                Value::LWord(word) => {
                    ensure_len(buffer, address.byte as usize + 7);
                    let bytes = word.to_le_bytes();
                    for (idx, byte) in bytes.iter().enumerate() {
                        buffer[address.byte as usize + idx] = *byte;
                    }
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
        }
    }

    fn area(&self, area: IoArea) -> &Vec<u8> {
        match area {
            IoArea::Input => &self.inputs,
            IoArea::Output => &self.outputs,
            IoArea::Memory => &self.memory,
        }
    }

    fn area_mut(&mut self, area: IoArea) -> &mut Vec<u8> {
        match area {
            IoArea::Input => &mut self.inputs,
            IoArea::Output => &mut self.outputs,
            IoArea::Memory => &mut self.memory,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IoAddressKey {
    area: IoArea,
    size: IoSize,
    path: Vec<u32>,
    bit: u8,
}

impl From<&IoAddress> for IoAddressKey {
    fn from(address: &IoAddress) -> Self {
        Self {
            area: address.area,
            size: address.size,
            path: address.path.clone(),
            bit: address.bit,
        }
    }
}

fn ensure_len(buffer: &mut Vec<u8>, index: usize) {
    if buffer.len() <= index {
        buffer.resize(index + 1, 0);
    }
}

fn parse_u32(value: Option<&str>, full: &str) -> Result<u32, RuntimeError> {
    value
        .ok_or_else(|| RuntimeError::InvalidIoAddress(full.into()))?
        .parse::<u32>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}

fn parse_u8(value: &str, full: &str) -> Result<u8, RuntimeError> {
    value
        .parse::<u8>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}

fn expected_size_for_type(value_type: TypeId) -> Option<IoSize> {
    match value_type {
        TypeId::BOOL => Some(IoSize::Bit),
        TypeId::SINT | TypeId::USINT | TypeId::BYTE | TypeId::CHAR => Some(IoSize::Byte),
        TypeId::INT | TypeId::UINT | TypeId::WORD | TypeId::WCHAR => Some(IoSize::Word),
        TypeId::DINT | TypeId::UDINT | TypeId::DWORD | TypeId::REAL => Some(IoSize::DWord),
        TypeId::LINT | TypeId::ULINT | TypeId::LWORD | TypeId::LREAL => Some(IoSize::LWord),
        _ => None,
    }
}

fn coerce_from_io(value: Value, target: TypeId) -> Result<Value, RuntimeError> {
    match target {
        TypeId::BOOL => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::SINT => match value {
            Value::Byte(byte) => Ok(Value::SInt(byte as i8)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::USINT => match value {
            Value::Byte(byte) => Ok(Value::USInt(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::BYTE => match value {
            Value::Byte(byte) => Ok(Value::Byte(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::CHAR => match value {
            Value::Byte(byte) => Ok(Value::Char(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::INT => match value {
            Value::Word(word) => Ok(Value::Int(word as i16)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::UINT => match value {
            Value::Word(word) => Ok(Value::UInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WORD => match value {
            Value::Word(word) => Ok(Value::Word(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WCHAR => match value {
            Value::Word(word) => Ok(Value::WChar(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DINT => match value {
            Value::DWord(word) => Ok(Value::DInt(word as i32)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::UDINT => match value {
            Value::DWord(word) => Ok(Value::UDInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DWORD => match value {
            Value::DWord(word) => Ok(Value::DWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::REAL => match value {
            Value::DWord(word) => Ok(Value::Real(f32::from_bits(word))),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LINT => match value {
            Value::LWord(word) => Ok(Value::LInt(word as i64)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::ULINT => match value {
            Value::LWord(word) => Ok(Value::ULInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LWORD => match value {
            Value::LWord(word) => Ok(Value::LWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LREAL => match value {
            Value::LWord(word) => Ok(Value::LReal(f64::from_bits(word))),
            _ => Err(RuntimeError::TypeMismatch),
        },
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn coerce_to_io(value: Value, target: TypeId, size: IoSize) -> Result<Value, RuntimeError> {
    let Some(expected) = expected_size_for_type(target) else {
        return Err(RuntimeError::TypeMismatch);
    };
    if expected != size {
        return Err(RuntimeError::TypeMismatch);
    }
    match target {
        TypeId::BOOL => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::SINT => {
            let val = match value {
                Value::SInt(val) => val,
                _ => i8::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val as u8))
        }
        TypeId::USINT => {
            let val = match value {
                Value::USInt(val) => val,
                _ => u8::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val))
        }
        TypeId::BYTE => match value {
            Value::Byte(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::CHAR => match value {
            Value::Char(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::INT => {
            let val = match value {
                Value::Int(val) => val,
                _ => i16::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val as u16))
        }
        TypeId::UINT => {
            let val = match value {
                Value::UInt(val) => val,
                _ => u16::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val))
        }
        TypeId::WORD => match value {
            Value::Word(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WCHAR => match value {
            Value::WChar(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DINT => {
            let val = match value {
                Value::DInt(val) => val,
                _ => i32::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val as u32))
        }
        TypeId::UDINT => {
            let val = match value {
                Value::UDInt(val) => val,
                _ => u32::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val))
        }
        TypeId::DWORD => match value {
            Value::DWord(val) => Ok(Value::DWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::REAL => {
            let val = match value {
                Value::Real(val) => val,
                _ => crate::numeric::to_f64(&value)? as f32,
            };
            Ok(Value::DWord(val.to_bits()))
        }
        TypeId::LINT => {
            let val = match value {
                Value::LInt(val) => val,
                _ => crate::numeric::to_i64(&value)?,
            };
            Ok(Value::LWord(val as u64))
        }
        TypeId::ULINT => {
            let val = match value {
                Value::ULInt(val) => val,
                _ => crate::numeric::to_u64(&value)?,
            };
            Ok(Value::LWord(val))
        }
        TypeId::LWORD => match value {
            Value::LWord(val) => Ok(Value::LWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LREAL => {
            let val = match value {
                Value::LReal(val) => val,
                _ => crate::numeric::to_f64(&value)?,
            };
            Ok(Value::LWord(val.to_bits()))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}
