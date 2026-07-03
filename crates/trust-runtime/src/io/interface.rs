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
                source: binding.source.clone(),
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
            source: None,
        });
    }

    pub fn bind_ref(&mut self, reference: ValueRef, address: IoAddress) {
        self.bindings.push(IoBinding {
            target: IoTarget::Reference(reference),
            address,
            value_type: None,
            display_name: None,
            source: None,
        });
    }

    pub fn bind_typed(&mut self, name: impl Into<SmolStr>, address: IoAddress, value_type: TypeId) {
        let name = name.into();
        self.bindings.push(IoBinding {
            target: IoTarget::Name(name.clone()),
            address,
            value_type: Some(value_type),
            display_name: Some(name),
            source: None,
        });
    }

    pub fn bind_ref_typed(&mut self, reference: ValueRef, address: IoAddress, value_type: TypeId) {
        self.bindings.push(IoBinding {
            target: IoTarget::Reference(reference),
            address,
            value_type: Some(value_type),
            display_name: None,
            source: None,
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
            source: None,
        });
    }

    pub fn set_binding_sources<F>(&mut self, mut source_for: F)
    where
        F: FnMut(&IoAddress) -> Option<SmolStr>,
    {
        for binding in &mut self.bindings {
            binding.source = source_for(&binding.address);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_carries_optional_binding_source() {
        let mut interface = IoInterface::new();
        interface.bind("Input0", IoAddress::parse("%IX0.0").expect("address"));
        interface.set_binding_sources(|address| {
            if matches!(address.area, IoArea::Input) {
                Some(SmolStr::new("Modbus 127.0.0.1:1502 · input reg 0"))
            } else {
                None
            }
        });

        let snapshot = interface.snapshot();
        assert_eq!(snapshot.inputs.len(), 1);
        assert_eq!(
            snapshot.inputs[0].source.as_deref(),
            Some("Modbus 127.0.0.1:1502 · input reg 0")
        );
    }
}
