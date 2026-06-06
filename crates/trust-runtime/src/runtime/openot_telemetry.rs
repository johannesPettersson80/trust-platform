use crate::config::OpenOtTelemetryConfig;
use crate::error::RuntimeError;

#[cfg(unix)]
mod imp {
    use super::*;
    use crate::config::{OpenOtTelemetryFenceMode, OpenOtTelemetrySource};
    use crate::memory::VariableStorage;
    use crate::value::Value;
    use open_ot_carriage::registry::{EVENT_HEARTBEAT, SYSTEM_SOURCE_ID};
    use open_ot_carriage::wire::{validate_record, Record, CRC_LEN, HEADER_LEN};
    use open_ot_shm::{FenceMode, SharedRecordPublisher};
    use smol_str::SmolStr;
    use std::path::Path;

    const ST_SCAN_RECORD_CAPACITY: usize = 1024;
    const ST_SCAN_RECORD_DESCRIPTOR_COUNT: usize = 17;

    #[derive(Debug)]
    pub(crate) struct OpenOtTelemetrySubsystem {
        publisher: Option<SharedRecordPublisher>,
        source: OpenOtTelemetrySource,
        producer_instance: Option<SmolStr>,
        previous_published_record_count: u64,
    }

    impl Default for OpenOtTelemetrySubsystem {
        fn default() -> Self {
            Self {
                publisher: None,
                source: OpenOtTelemetrySource::Heartbeat,
                producer_instance: None,
                previous_published_record_count: 0,
            }
        }
    }

    impl OpenOtTelemetrySubsystem {
        pub(crate) fn configure(
            &mut self,
            config: &OpenOtTelemetryConfig,
            bundle_root: Option<&Path>,
        ) -> Result<(), RuntimeError> {
            if !config.enabled {
                self.publisher = None;
                self.source = OpenOtTelemetrySource::Heartbeat;
                self.producer_instance = None;
                self.previous_published_record_count = 0;
                return Ok(());
            }
            validate_config(config)?;
            let path = resolve_path(&config.path, bundle_root);
            let publisher =
                SharedRecordPublisher::create_with_mode(&path, config.capacity, fence_mode(config))
                    .map_err(|err| telemetry_error("create", err))?;
            self.publisher = Some(publisher);
            self.source = config.source;
            self.producer_instance = config.producer_instance.clone();
            self.previous_published_record_count = 0;
            Ok(())
        }

        #[must_use]
        pub(crate) fn is_enabled(&self) -> bool {
            self.publisher.is_some()
        }

        pub(crate) fn publish(
            &mut self,
            cycle_counter: u64,
            storage: &VariableStorage,
        ) -> Result<(), RuntimeError> {
            match self.source {
                OpenOtTelemetrySource::Heartbeat => self.publish_heartbeat(cycle_counter),
                OpenOtTelemetrySource::StFb => self.publish_st_fb(storage),
            }
        }

        pub(crate) fn reset_scan_state(&mut self) {
            self.previous_published_record_count = 0;
        }

        fn publish_heartbeat(&mut self, cycle_counter: u64) -> Result<(), RuntimeError> {
            let Some(publisher) = self.publisher.as_mut() else {
                return Ok(());
            };
            let record = Record::new(
                cycle_counter,
                1,
                cycle_counter,
                SYSTEM_SOURCE_ID,
                EVENT_HEARTBEAT,
            );
            publisher
                .append_record(&record)
                .map_err(|err| telemetry_error("publish heartbeat", err))
        }

        fn publish_st_fb(&mut self, storage: &VariableStorage) -> Result<(), RuntimeError> {
            let Some(_) = self.publisher.as_ref() else {
                return Ok(());
            };
            let path = self.producer_instance.as_deref().ok_or_else(|| {
                telemetry_message(
                    "st-fb source requires runtime.openot.producer_instance to be configured",
                )
            })?;
            let snapshot = read_st_fb_outputs(storage, path)?;
            let delta = snapshot
                .published_record_count
                .checked_sub(self.previous_published_record_count)
                .ok_or_else(|| {
                    telemetry_message(format!(
                        "producer '{path}' PublishedRecordCount moved backwards from {} to {}",
                        self.previous_published_record_count, snapshot.published_record_count
                    ))
                })?;

            if delta == 0 {
                return Ok(());
            }

            let scan_record_count =
                u64::try_from(snapshot.scan_record_count).expect("UINT count fits u64");
            if delta != scan_record_count {
                return Err(telemetry_message(format!(
                    "producer '{path}' delta {delta} != ScanRecordCount {}",
                    snapshot.scan_record_count
                )));
            }

            let publisher_capacity = self
                .publisher
                .as_ref()
                .expect("publisher checked above")
                .capacity();
            let records = validate_scan_record_descriptors(path, &snapshot, publisher_capacity)?;
            let publisher = self.publisher.as_mut().expect("publisher checked above");
            for encoded in records {
                publisher
                    .append_encoded(encoded)
                    .map_err(|err| telemetry_error("publish st-fb record", err))?;
            }
            self.previous_published_record_count = snapshot.published_record_count;
            Ok(())
        }
    }

    fn validate_config(config: &OpenOtTelemetryConfig) -> Result<(), RuntimeError> {
        if config.path.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.path must not be empty when runtime.openot.enabled=true".into(),
            ));
        }
        if config.capacity == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.capacity must be >= 1".into(),
            ));
        }
        if config.fence_mode == OpenOtTelemetryFenceMode::Unfenced
            && !config.allow_unfenced_for_proof
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.fence_mode='unfenced' requires runtime.openot.allow_unfenced_for_proof=true"
                    .into(),
            ));
        }
        match config.source {
            OpenOtTelemetrySource::Heartbeat => {
                if config.producer_instance.is_some() {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.openot.producer_instance is only valid when runtime.openot.source='st-fb'"
                            .into(),
                    ));
                }
            }
            OpenOtTelemetrySource::StFb => {
                let Some(path) = config.producer_instance.as_deref() else {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.openot.producer_instance is required when runtime.openot.source='st-fb'"
                            .into(),
                    ));
                };
                if !is_qualified_producer_path(path) {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.openot.producer_instance must be a qualified path like 'Main.Producer'"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn is_qualified_producer_path(path: &str) -> bool {
        let mut parts = path.split('.');
        let Some(program) = parts.next() else {
            return false;
        };
        let Some(instance) = parts.next() else {
            return false;
        };
        parts.next().is_none()
            && !program.trim().is_empty()
            && !instance.trim().is_empty()
            && program == program.trim()
            && instance == instance.trim()
    }

    fn resolve_path(path: &Path, bundle_root: Option<&Path>) -> std::path::PathBuf {
        if path.is_relative() {
            bundle_root.unwrap_or_else(|| Path::new(".")).join(path)
        } else {
            path.to_path_buf()
        }
    }

    fn fence_mode(config: &OpenOtTelemetryConfig) -> FenceMode {
        match config.fence_mode {
            OpenOtTelemetryFenceMode::Fenced => FenceMode::Fenced,
            OpenOtTelemetryFenceMode::Unfenced => FenceMode::Unfenced,
        }
    }

    fn telemetry_error(action: &str, err: impl std::fmt::Debug) -> RuntimeError {
        RuntimeError::IoTransport(SmolStr::new(format!(
            "OpenOT telemetry {action} failed: {err:?}"
        )))
    }

    fn telemetry_message(message: impl Into<String>) -> RuntimeError {
        RuntimeError::IoTransport(SmolStr::new(format!("OpenOT telemetry {}", message.into())))
    }

    #[derive(Debug)]
    struct StFbProducerSnapshot {
        published_record_count: u64,
        scan_records: [u8; ST_SCAN_RECORD_CAPACITY],
        scan_record_offsets: [usize; ST_SCAN_RECORD_DESCRIPTOR_COUNT],
        scan_record_lengths: [usize; ST_SCAN_RECORD_DESCRIPTOR_COUNT],
        scan_record_count: usize,
    }

    fn read_st_fb_outputs(
        storage: &VariableStorage,
        path: &str,
    ) -> Result<StFbProducerSnapshot, RuntimeError> {
        let (program, producer) = path.split_once('.').ok_or_else(|| {
            telemetry_message(format!(
                "producer_instance '{path}' must be a qualified path like 'Main.Producer'"
            ))
        })?;
        if program.is_empty() || producer.is_empty() || producer.contains('.') {
            return Err(telemetry_message(format!(
                "producer_instance '{path}' must be a qualified path like 'Main.Producer'"
            )));
        }

        let program_id = match storage.get_global(program) {
            Some(Value::Instance(id)) => *id,
            Some(other) => {
                return Err(telemetry_message(format!(
                    "producer path '{path}' expected program '{program}' to be an instance, got {}",
                    value_type_name(other)
                )));
            }
            None => {
                return Err(telemetry_message(format!(
                    "producer path '{path}' program '{program}' was not found"
                )));
            }
        };

        let producer_id = match storage.get_instance_var_recursive(program_id, producer) {
            Some(Value::Instance(id)) => *id,
            Some(other) => {
                return Err(telemetry_message(format!(
                    "producer path '{path}' expected '{producer}' to be an FB instance, got {}",
                    value_type_name(other)
                )));
            }
            None => {
                return Err(telemetry_message(format!(
                    "producer path '{path}' field '{producer}' was not found"
                )));
            }
        };

        Ok(StFbProducerSnapshot {
            published_record_count: read_ulint_field(
                storage,
                producer_id,
                path,
                "PublishedRecordCount",
            )?,
            scan_records: read_byte_array_field::<ST_SCAN_RECORD_CAPACITY>(
                storage,
                producer_id,
                path,
                "ScanRecords",
                (0, 1023),
                "ARRAY[0..1023] OF BYTE",
            )?,
            scan_record_offsets: read_uint_array_field::<ST_SCAN_RECORD_DESCRIPTOR_COUNT>(
                storage,
                producer_id,
                path,
                "ScanRecordOffsets",
                (0, 16),
                "ARRAY[0..16] OF UINT",
            )?,
            scan_record_lengths: read_uint_array_field::<ST_SCAN_RECORD_DESCRIPTOR_COUNT>(
                storage,
                producer_id,
                path,
                "ScanRecordLengths",
                (0, 16),
                "ARRAY[0..16] OF UINT",
            )?,
            scan_record_count: read_uint_field(storage, producer_id, path, "ScanRecordCount")?,
        })
    }

    fn read_ulint_field(
        storage: &VariableStorage,
        producer_id: crate::memory::InstanceId,
        path: &str,
        field: &str,
    ) -> Result<u64, RuntimeError> {
        match storage.get_instance_var_recursive(producer_id, field) {
            Some(Value::ULInt(value)) => Ok(*value),
            Some(other) => Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected ULINT, got {}",
                value_type_name(other)
            ))),
            None => Err(telemetry_message(format!(
                "producer '{path}' field '{field}' was not found"
            ))),
        }
    }

    fn read_uint_field(
        storage: &VariableStorage,
        producer_id: crate::memory::InstanceId,
        path: &str,
        field: &str,
    ) -> Result<usize, RuntimeError> {
        match storage.get_instance_var_recursive(producer_id, field) {
            Some(Value::UInt(value)) => Ok(usize::from(*value)),
            Some(other) => Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected UINT, got {}",
                value_type_name(other)
            ))),
            None => Err(telemetry_message(format!(
                "producer '{path}' field '{field}' was not found"
            ))),
        }
    }

    fn read_byte_array_field<const N: usize>(
        storage: &VariableStorage,
        producer_id: crate::memory::InstanceId,
        path: &str,
        field: &str,
        dimensions: (i64, i64),
        expected_type: &str,
    ) -> Result<[u8; N], RuntimeError> {
        let value = storage
            .get_instance_var_recursive(producer_id, field)
            .ok_or_else(|| {
                telemetry_message(format!("producer '{path}' field '{field}' was not found"))
            })?;
        let Value::Array(array) = value else {
            return Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected {expected_type}, got {}",
                value_type_name(value)
            )));
        };
        if array.dimensions() != [dimensions] || array.elements().len() != N {
            return Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected {expected_type}, got dimensions {:?} with {} elements",
                array.dimensions(),
                array.elements().len()
            )));
        }

        let mut bytes = [0u8; N];
        for (index, value) in array.elements().iter().enumerate() {
            let Value::Byte(byte) = value else {
                return Err(telemetry_message(format!(
                    "producer '{path}' field '{field}[{index}]' expected BYTE, got {}",
                    value_type_name(value)
                )));
            };
            bytes[index] = *byte;
        }
        Ok(bytes)
    }

    fn read_uint_array_field<const N: usize>(
        storage: &VariableStorage,
        producer_id: crate::memory::InstanceId,
        path: &str,
        field: &str,
        dimensions: (i64, i64),
        expected_type: &str,
    ) -> Result<[usize; N], RuntimeError> {
        let value = storage
            .get_instance_var_recursive(producer_id, field)
            .ok_or_else(|| {
                telemetry_message(format!("producer '{path}' field '{field}' was not found"))
            })?;
        let Value::Array(array) = value else {
            return Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected {expected_type}, got {}",
                value_type_name(value)
            )));
        };
        if array.dimensions() != [dimensions] || array.elements().len() != N {
            return Err(telemetry_message(format!(
                "producer '{path}' field '{field}' expected {expected_type}, got dimensions {:?} with {} elements",
                array.dimensions(),
                array.elements().len()
            )));
        }

        let mut values = [0usize; N];
        for (index, value) in array.elements().iter().enumerate() {
            let Value::UInt(uint) = value else {
                return Err(telemetry_message(format!(
                    "producer '{path}' field '{field}[{index}]' expected UINT, got {}",
                    value_type_name(value)
                )));
            };
            values[index] = usize::from(*uint);
        }
        Ok(values)
    }

    fn validate_scan_record_descriptors<'a>(
        path: &str,
        snapshot: &'a StFbProducerSnapshot,
        publisher_capacity: usize,
    ) -> Result<Vec<&'a [u8]>, RuntimeError> {
        if snapshot.scan_record_count > ST_SCAN_RECORD_DESCRIPTOR_COUNT {
            return Err(telemetry_message(format!(
                "producer '{path}' ScanRecordCount {} exceeds descriptor capacity {ST_SCAN_RECORD_DESCRIPTOR_COUNT}",
                snapshot.scan_record_count
            )));
        }

        let mut records = Vec::with_capacity(snapshot.scan_record_count);
        for index in 0..snapshot.scan_record_count {
            let offset = snapshot.scan_record_offsets[index];
            let len = snapshot.scan_record_lengths[index];
            if len < HEADER_LEN + CRC_LEN {
                return Err(telemetry_message(format!(
                    "producer '{path}' ScanRecordLengths[{index}]={len} is shorter than minimum record length {}",
                    HEADER_LEN + CRC_LEN
                )));
            }
            let end = offset.checked_add(len).ok_or_else(|| {
                telemetry_message(format!(
                    "producer '{path}' ScanRecordOffsets[{index}]={offset} plus length {len} overflows"
                ))
            })?;
            if end > ST_SCAN_RECORD_CAPACITY {
                return Err(telemetry_message(format!(
                    "producer '{path}' ScanRecords descriptor {index} window [{offset}..{end}) exceeds capacity {ST_SCAN_RECORD_CAPACITY}"
                )));
            }
            if len > publisher_capacity {
                return Err(telemetry_message(format!(
                    "producer '{path}' ScanRecordLengths[{index}]={len} exceeds telemetry ring capacity {publisher_capacity}"
                )));
            }

            let record = &snapshot.scan_records[offset..end];
            let consumed = validate_record(record).map_err(|err| {
                telemetry_message(format!(
                    "producer '{path}' ScanRecords descriptor {index} is not a valid OpenOT record: {err:?}"
                ))
            })?;
            if consumed != len {
                return Err(telemetry_message(format!(
                    "producer '{path}' ScanRecords descriptor {index} declared length {consumed} != descriptor length {len}"
                )));
            }
            records.push(record);
        }
        Ok(records)
    }

    fn value_type_name(value: &Value) -> &'static str {
        match value {
            Value::Bool(_) => "BOOL",
            Value::SInt(_) => "SINT",
            Value::Int(_) => "INT",
            Value::DInt(_) => "DINT",
            Value::LInt(_) => "LINT",
            Value::USInt(_) => "USINT",
            Value::UInt(_) => "UINT",
            Value::UDInt(_) => "UDINT",
            Value::ULInt(_) => "ULINT",
            Value::Real(_) => "REAL",
            Value::LReal(_) => "LREAL",
            Value::Byte(_) => "BYTE",
            Value::Word(_) => "WORD",
            Value::DWord(_) => "DWORD",
            Value::LWord(_) => "LWORD",
            Value::Time(_) => "TIME",
            Value::LTime(_) => "LTIME",
            Value::Date(_) => "DATE",
            Value::LDate(_) => "LDATE",
            Value::Tod(_) => "TOD",
            Value::LTod(_) => "LTOD",
            Value::Dt(_) => "DT",
            Value::Ldt(_) => "LDT",
            Value::String(_) => "STRING",
            Value::WString(_) => "WSTRING",
            Value::Char(_) => "CHAR",
            Value::WChar(_) => "WCHAR",
            Value::Array(_) => "ARRAY",
            Value::Struct(_) => "STRUCT",
            Value::Enum(_) => "ENUM",
            Value::Reference(_) => "REFERENCE",
            Value::Instance(_) => "INSTANCE",
            Value::Null => "NULL",
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;
    use std::path::Path;

    #[derive(Debug, Default)]
    pub(crate) struct OpenOtTelemetrySubsystem;

    impl OpenOtTelemetrySubsystem {
        pub(crate) fn configure(
            &mut self,
            config: &OpenOtTelemetryConfig,
            _bundle_root: Option<&Path>,
        ) -> Result<(), RuntimeError> {
            if config.enabled {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot shared-memory telemetry is only supported on Unix targets"
                        .into(),
                ));
            }
            Ok(())
        }

        #[must_use]
        pub(crate) fn is_enabled(&self) -> bool {
            false
        }

        pub(crate) fn publish(
            &mut self,
            _cycle_counter: u64,
            _storage: &crate::memory::VariableStorage,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }

        pub(crate) fn reset_scan_state(&mut self) {}
    }
}

pub(super) use imp::OpenOtTelemetrySubsystem;
