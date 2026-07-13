#[derive(Debug, Default)]
pub struct IoInterface {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
    bindings: Vec<IoBinding>,
    hierarchical: std::collections::HashMap<IoAddressKey, Value>,
}

pub const PROCESS_IMAGE_AREA_LIMIT: usize = 16 * 1024 * 1024;

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
        self.try_resize(inputs, outputs, memory)
            .expect("process image resize exceeds runtime cap");
    }

    /// Resize the process image buffers with runtime cap validation.
    pub fn try_resize(
        &mut self,
        inputs: usize,
        outputs: usize,
        memory: usize,
    ) -> Result<(), RuntimeError> {
        validate_process_image_area_len("input", inputs)?;
        validate_process_image_area_len("output", outputs)?;
        validate_process_image_area_len("memory", memory)?;
        self.inputs.resize(inputs, 0);
        self.outputs.resize(outputs, 0);
        self.memory.resize(memory, 0);
        Ok(())
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
                    Ok(value) => {
                        let value = if let Some(value_type) = binding.value_type {
                            coerce_from_io(value, value_type)
                        } else {
                            Ok(value)
                        };
                        match value {
                            Ok(value) => IoSnapshotValue::Value(value),
                            Err(err) => IoSnapshotValue::Error(err.to_string()),
                        }
                    }
                    Err(err) => IoSnapshotValue::Error(err.to_string()),
                }
            };
            let entry = IoSnapshotEntry {
                name,
                address: binding.address.clone(),
                value_type: binding.value_type,
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
            IoSize::Bytes(len) => {
                let start = address.byte as usize;
                let end = start.saturating_add(len as usize);
                let bytes = if start < buffer.len() {
                    &buffer[start..buffer.len().min(end)]
                } else {
                    &[]
                };
                let nul = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
                let text = std::str::from_utf8(&bytes[..nul]).map_err(|_| RuntimeError::TypeMismatch)?;
                Ok(Value::String(SmolStr::new(text)))
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
                    ensure_len(buffer, address.byte as usize)?;
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
                    ensure_len(buffer, address.byte as usize)?;
                    buffer[address.byte as usize] = byte;
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::Word => match value {
                Value::Word(word) => {
                    ensure_len(buffer, address.byte as usize + 1)?;
                    let [lo, hi] = word.to_le_bytes();
                    buffer[address.byte as usize] = lo;
                    buffer[address.byte as usize + 1] = hi;
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::DWord => match value {
                Value::DWord(word) => {
                    ensure_len(buffer, address.byte as usize + 3)?;
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
                    ensure_len(buffer, address.byte as usize + 7)?;
                    let bytes = word.to_le_bytes();
                    for (idx, byte) in bytes.iter().enumerate() {
                        buffer[address.byte as usize + idx] = *byte;
                    }
                    Ok(())
                }
                _ => Err(RuntimeError::TypeMismatch),
            },
            IoSize::Bytes(len) => match value {
                Value::String(text) => {
                    let bytes = text.as_bytes();
                    if bytes.len() > len as usize {
                        return Err(RuntimeError::Overflow);
                    }
                    let start = address.byte as usize;
                    let end = start.checked_add(len as usize).ok_or_else(|| {
                        RuntimeError::InvalidIoAddress("process image address overflow".into())
                    })?;
                    ensure_len(buffer, end.saturating_sub(1))?;
                    buffer[start..end].fill(0);
                    buffer[start..start + bytes.len()].copy_from_slice(bytes);
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

fn ensure_len(buffer: &mut Vec<u8>, index: usize) -> Result<(), RuntimeError> {
    if index >= PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error("process image address");
    }
    if buffer.len() <= index {
        buffer.resize(index + 1, 0);
    }
    Ok(())
}

pub fn validate_process_image_address(address: &IoAddress) -> Result<(), RuntimeError> {
    if address.wildcard || address.path.len() > 1 {
        return Ok(());
    }
    let byte = address.byte as usize;
    let extra = match address.size {
        IoSize::Bit | IoSize::Byte => 0,
        IoSize::Word => 1,
        IoSize::DWord => 3,
        IoSize::LWord => 7,
        IoSize::Bytes(len) => len.saturating_sub(1) as usize,
    };
    let last_byte = byte.checked_add(extra).ok_or_else(|| {
        RuntimeError::InvalidIoAddress("process image address overflow".into())
    })?;
    if last_byte >= PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error("process image address");
    }
    Ok(())
}

fn validate_process_image_area_len(area: &str, len: usize) -> Result<(), RuntimeError> {
    if len > PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error(area);
    }
    Ok(())
}

fn process_image_area_limit_error(area: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::InvalidIoAddress(
        format!(
            "{area} exceeds {} byte process image area limit",
            PROCESS_IMAGE_AREA_LIMIT
        )
        .into(),
    ))
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

    #[test]
    fn snapshot_carries_and_formats_typed_bindings() {
        let mut interface = IoInterface::new();
        interface.bind_typed(
            "Speed",
            IoAddress::parse("%MD0").expect("real address"),
            TypeId::REAL,
        );
        interface.bind_typed(
            "Delay",
            IoAddress::parse("%MD4").expect("time address"),
            TypeId::TIME,
        );
        let mut label_address = IoAddress::parse("%IB8").expect("string address");
        label_address.size = IoSize::Bytes(12);
        interface.bind_typed("Label", label_address.clone(), TypeId::STRING);
        interface
            .write(&IoAddress::parse("%MD0").expect("real address"), Value::DWord(0x3FC0_0000))
            .expect("write REAL bits");
        interface
            .write(&IoAddress::parse("%MD4").expect("time address"), Value::DWord(250))
            .expect("write TIME millis");
        interface
            .write(&label_address, Value::String(SmolStr::new("Ready")))
            .expect("write STRING bytes");

        let snapshot = interface.snapshot();

        assert_eq!(snapshot.memory[0].value_type, Some(TypeId::REAL));
        match &snapshot.memory[0].value {
            IoSnapshotValue::Value(Value::Real(value)) => assert_eq!(*value, 1.5),
            other => panic!("expected REAL snapshot value, got {other:?}"),
        }
        assert_eq!(snapshot.memory[1].value_type, Some(TypeId::TIME));
        match &snapshot.memory[1].value {
            IoSnapshotValue::Value(Value::Time(value)) => {
                assert_eq!(*value, crate::value::Duration::from_millis(250));
            }
            other => panic!("expected TIME snapshot value, got {other:?}"),
        }
        assert_eq!(snapshot.inputs[0].value_type, Some(TypeId::STRING));
        match &snapshot.inputs[0].value {
            IoSnapshotValue::Value(Value::String(value)) => assert_eq!(value.as_str(), "Ready"),
            other => panic!("expected STRING snapshot value, got {other:?}"),
        }
    }

    #[test]
    fn snapshot_rejects_non_finite_real_input_bits() {
        let mut interface = IoInterface::new();
        interface.bind_typed(
            "Temperature",
            IoAddress::parse("%MD0").expect("real address"),
            TypeId::REAL,
        );
        interface
            .write(
                &IoAddress::parse("%MD0").expect("real address"),
                Value::DWord(f32::NAN.to_bits()),
            )
            .expect("write raw REAL bits");

        let snapshot = interface.snapshot();

        match &snapshot.memory[0].value {
            IoSnapshotValue::Error(error) => assert!(
                error.contains("finite"),
                "expected finite-value diagnostic, got {error}"
            ),
            other => panic!("expected non-finite REAL bits to be rejected, got {other:?}"),
        }
    }

    #[test]
    fn typed_real_output_rejects_nonfinite_and_narrowing_overflow_without_write() {
        for value in [
            Value::Real(f32::NAN),
            Value::Real(f32::INFINITY),
            Value::Real(f32::NEG_INFINITY),
            Value::LReal(f64::MAX),
        ] {
            let mut interface = IoInterface::new();
            interface.resize(0, 4, 0);
            interface.outputs_mut().fill(0xA5);
            interface.bind_typed(
                "Output",
                IoAddress::parse("%QD0").expect("REAL output address"),
                TypeId::REAL,
            );
            let mut storage = VariableStorage::new();
            storage.set_global("Output", value.clone());

            let err = interface
                .write_outputs(&storage)
                .expect_err("non-finite typed REAL output must fail");

            assert_eq!(
                err,
                RuntimeError::IoDriver(
                    "typed REAL process-image value must be finite".into()
                ),
                "{value:?}"
            );
            assert_eq!(interface.outputs(), &[0xA5; 4], "{value:?}");
        }
    }

    #[test]
    fn typed_lreal_output_rejection_is_transactional() {
        let mut interface = IoInterface::new();
        interface.resize(0, 12, 0);
        interface.outputs_mut().fill(0x5A);
        interface.bind_typed(
            "Finite",
            IoAddress::parse("%QD0").expect("REAL output address"),
            TypeId::REAL,
        );
        interface.bind_typed(
            "Invalid",
            IoAddress::parse("%QL4").expect("LREAL output address"),
            TypeId::LREAL,
        );
        let mut storage = VariableStorage::new();
        storage.set_global("Finite", Value::Real(1.25));
        storage.set_global("Invalid", Value::LReal(f64::INFINITY));

        let err = interface
            .write_outputs(&storage)
            .expect_err("non-finite typed LREAL output must fail");

        assert_eq!(
            err,
            RuntimeError::IoDriver("typed LREAL process-image value must be finite".into())
        );
        assert_eq!(interface.outputs(), &[0x5A; 12]);
    }

    #[test]
    fn string_process_image_write_rejects_payload_larger_than_declared_window() {
        let mut interface = IoInterface::new();
        let mut text = IoAddress::parse("%QB0").expect("output byte address");
        text.size = IoSize::Bytes(3);

        let err = interface
            .write(&text, Value::String(SmolStr::new("ABCD")))
            .expect_err("overlong STRING process-image write must fail");
        assert_eq!(err, RuntimeError::Overflow);
        assert!(
            interface.outputs().is_empty(),
            "failed overlong write must not allocate or partially mutate output image"
        );
    }

    #[test]
    fn process_image_write_rejects_addresses_above_area_cap() {
        let mut interface = IoInterface::new();
        let mut max_address = IoAddress::parse("%MB0").expect("memory byte address");
        max_address.byte = (PROCESS_IMAGE_AREA_LIMIT - 1) as u32;
        interface
            .write(&max_address, Value::Byte(0xAA))
            .expect("last byte inside area cap should be writable");
        assert_eq!(interface.memory().len(), PROCESS_IMAGE_AREA_LIMIT);

        let mut too_large = IoAddress::parse("%MB0").expect("memory byte address");
        too_large.byte = PROCESS_IMAGE_AREA_LIMIT as u32;
        let err = interface
            .write(&too_large, Value::Byte(0xBB))
            .expect_err("address above area cap must fail");
        assert!(
            err.to_string().contains("area limit"),
            "expected cap error, got {err}"
        );
    }

    #[test]
    fn process_image_try_resize_rejects_areas_above_cap() {
        let mut interface = IoInterface::new();
        interface
            .try_resize(PROCESS_IMAGE_AREA_LIMIT, 1, 1)
            .expect("area exactly at cap should resize");
        let err = interface
            .try_resize(PROCESS_IMAGE_AREA_LIMIT + 1, 1, 1)
            .expect_err("area above cap must fail");
        assert!(
            err.to_string().contains("process image area limit"),
            "expected cap error, got {err}"
        );
    }

    #[test]
    fn process_image_over_reads_are_zero_filled_and_do_not_resize() {
        let interface = IoInterface::new();
        let mut byte = IoAddress::parse("%MB0").expect("memory byte address");
        byte.byte = PROCESS_IMAGE_AREA_LIMIT as u32;
        assert_eq!(
            interface.read(&byte).expect("read over cap byte"),
            Value::Byte(0)
        );
        assert_eq!(
            interface.memory().len(),
            0,
            "over-reading must not allocate memory image bytes"
        );

        let mut word = IoAddress::parse("%IW0").expect("input word address");
        word.byte = PROCESS_IMAGE_AREA_LIMIT as u32;
        assert_eq!(
            interface.read(&word).expect("read over cap word"),
            Value::Word(0)
        );
        assert_eq!(
            interface.inputs().len(),
            0,
            "over-reading must not allocate input image bytes"
        );

        let mut text = IoAddress::parse("%QB0").expect("output byte address");
        text.byte = PROCESS_IMAGE_AREA_LIMIT as u32;
        text.size = IoSize::Bytes(16);
        assert_eq!(
            interface.read(&text).expect("read over cap string"),
            Value::String(SmolStr::new(""))
        );
        assert_eq!(
            interface.outputs().len(),
            0,
            "over-reading must not allocate output image bytes"
        );
    }
}
