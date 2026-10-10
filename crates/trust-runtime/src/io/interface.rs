#[derive(Debug, Default)]
pub struct IoInterface {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
    bindings: Vec<IoBinding>,
    binding_codecs: Vec<IoBindingCodec>,
    hierarchical: std::collections::HashMap<IoAddressKey, Value>,
}

pub use trust_runtime_core::io_address::PROCESS_IMAGE_AREA_LIMIT;

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
        for (binding, codec) in self.bindings.iter().zip(&self.binding_codecs) {
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
                        let value = coerce_binding_from_io(value, binding, codec);
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
                value_type_name: codec
                    .enum_type
                    .as_ref()
                    .map(|enum_type| enum_type.type_name.clone()),
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
        self.push_binding(
            IoBinding {
                target: IoTarget::Name(name.clone()),
                address,
                value_type: None,
                display_name: Some(name),
                source: None,
            },
            IoBindingCodec::default(),
        );
    }

    pub fn bind_ref(&mut self, reference: ValueRef, address: IoAddress) {
        self.push_binding(
            IoBinding {
                target: IoTarget::Reference(reference),
                address,
                value_type: None,
                display_name: None,
                source: None,
            },
            IoBindingCodec::default(),
        );
    }

    pub fn bind_typed(&mut self, name: impl Into<SmolStr>, address: IoAddress, value_type: TypeId) {
        let name = name.into();
        self.push_binding(
            IoBinding {
                target: IoTarget::Name(name.clone()),
                address,
                value_type: Some(value_type),
                display_name: Some(name),
                source: None,
            },
            IoBindingCodec {
                wire_type: Some(value_type),
                enum_type: None,
            },
        );
    }

    pub fn bind_ref_typed(&mut self, reference: ValueRef, address: IoAddress, value_type: TypeId) {
        self.push_binding(
            IoBinding {
                target: IoTarget::Reference(reference),
                address,
                value_type: Some(value_type),
                display_name: None,
                source: None,
            },
            IoBindingCodec {
                wire_type: Some(value_type),
                enum_type: None,
            },
        );
    }

    pub fn bind_ref_named_typed(
        &mut self,
        reference: ValueRef,
        address: IoAddress,
        value_type: TypeId,
        name: impl Into<SmolStr>,
    ) {
        self.push_binding(
            IoBinding {
                target: IoTarget::Reference(reference),
                address,
                value_type: Some(value_type),
                display_name: Some(name.into()),
                source: None,
            },
            IoBindingCodec {
                wire_type: Some(value_type),
                enum_type: None,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_ref_named_enum(
        &mut self,
        reference: ValueRef,
        address: IoAddress,
        enum_type: TypeId,
        wire_type: TypeId,
        type_name: SmolStr,
        variants: Vec<(SmolStr, i64)>,
        name: impl Into<SmolStr>,
    ) {
        self.push_binding(
            IoBinding {
                target: IoTarget::Reference(reference),
                address,
                value_type: Some(enum_type),
                display_name: Some(name.into()),
                source: None,
            },
            IoBindingCodec {
                wire_type: Some(wire_type),
                enum_type: Some(IoEnumBinding {
                    type_name,
                    variants,
                }),
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn configure_ref_named_enum(
        &mut self,
        reference: &ValueRef,
        address: &IoAddress,
        enum_type: TypeId,
        wire_type: TypeId,
        type_name: SmolStr,
        variants: Vec<(SmolStr, i64)>,
        name: impl Into<SmolStr>,
    ) -> bool {
        let Some(index) = self.bindings.iter().position(|binding| {
            binding.address == *address
                && matches!(&binding.target, IoTarget::Reference(candidate) if candidate == reference)
        }) else {
            return false;
        };
        let binding = &mut self.bindings[index];
        binding.value_type = Some(enum_type);
        self.binding_codecs[index] = IoBindingCodec {
            wire_type: Some(wire_type),
            enum_type: Some(IoEnumBinding {
                type_name,
                variants,
            }),
        };
        binding.display_name = Some(name.into());
        true
    }

    fn push_binding(&mut self, binding: IoBinding, codec: IoBindingCodec) {
        self.bindings.push(binding);
        self.binding_codecs.push(codec);
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
        let mut staged = storage.clone();
        for (binding, codec) in self.bindings.iter().zip(&self.binding_codecs) {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            let value = self.read(&binding.address)?;
            let value = coerce_binding_from_io(value, binding, codec)?;
            match &binding.target {
                IoTarget::Name(name) => staged.set_global(name.clone(), value),
                IoTarget::Reference(reference) => {
                    if !staged.write_by_ref(reference.clone(), value) {
                        return Err(RuntimeError::NullReference);
                    }
                }
            }
        }
        *storage = staged;
        Ok(())
    }

    pub fn write_outputs(&mut self, storage: &VariableStorage) -> Result<(), RuntimeError> {
        let bindings = self.bindings.clone();
        let codecs = self.binding_codecs.clone();
        let mut pending = Vec::new();
        for (binding, codec) in bindings.into_iter().zip(codecs) {
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
            let value = coerce_binding_to_io(value.clone(), &binding, &codec)?;
            pending.push((binding.address, value));
        }

        let mut staged = Self {
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            memory: self.memory.clone(),
            bindings: Vec::new(),
            binding_codecs: Vec::new(),
            hierarchical: self.hierarchical.clone(),
        };
        for (address, value) in pending {
            staged.write(&address, value)?;
        }
        self.outputs = staged.outputs;
        self.memory = staged.memory;
        self.hierarchical = staged.hierarchical;
        Ok(())
    }

    pub fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let key = IoAddressKey::from(address);
            return self.hierarchical.get(&key).cloned().ok_or_else(|| {
                RuntimeError::InvalidIoAddress(format!("hier {:?}", address.path).into())
            });
        }
        let buffer = self.area(address.area);
        trust_runtime_core::io_image::read_flat_image(buffer, address)
    }

    pub fn write(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let key = IoAddressKey::from(address);
            self.hierarchical.insert(key, value);
            return Ok(());
        }
        let buffer = self.area_mut(address.area);
        trust_runtime_core::io_image::write_flat_image(buffer, address, value)
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

pub use trust_runtime_core::io_image::validate_process_image_address;
use trust_runtime_core::io_image::{validate_process_image_area_len, IoAddressKey};

#[cfg(test)]
#[path = "interface_contract_tests.rs"]
mod interface_contract_tests;

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
            .write(
                &IoAddress::parse("%MD0").expect("real address"),
                Value::DWord(0x3FC0_0000),
            )
            .expect("write REAL bits");
        interface
            .write(
                &IoAddress::parse("%MD4").expect("time address"),
                Value::DWord(250),
            )
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
                RuntimeError::IoDriver("typed REAL process-image value must be finite".into()),
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
    fn process_image_in_range_unallocated_reads_are_zero_filled_and_do_not_resize() {
        let interface = IoInterface::new();
        let mut byte = IoAddress::parse("%MB0").expect("memory byte address");
        byte.byte = (PROCESS_IMAGE_AREA_LIMIT - 1) as u32;
        assert_eq!(
            interface.read(&byte).expect("read unallocated byte"),
            Value::Byte(0)
        );
        assert_eq!(
            interface.memory().len(),
            0,
            "unallocated reads must not allocate memory image bytes"
        );

        let mut word = IoAddress::parse("%IW0").expect("input word address");
        word.byte = (PROCESS_IMAGE_AREA_LIMIT - 2) as u32;
        assert_eq!(
            interface.read(&word).expect("read unallocated word"),
            Value::Word(0)
        );
        assert_eq!(
            interface.inputs().len(),
            0,
            "unallocated reads must not allocate input image bytes"
        );

        let mut text = IoAddress::parse("%QB0").expect("output byte address");
        text.byte = (PROCESS_IMAGE_AREA_LIMIT - 16) as u32;
        text.size = IoSize::Bytes(16);
        assert_eq!(
            interface.read(&text).expect("read unallocated string"),
            Value::String(SmolStr::new(""))
        );
        assert_eq!(
            interface.outputs().len(),
            0,
            "unallocated reads must not allocate output image bytes"
        );
    }
}
