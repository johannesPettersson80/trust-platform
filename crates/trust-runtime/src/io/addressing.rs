pub use trust_runtime_core::io_address::{IoAddress, IoSize};

#[derive(Debug, Clone)]
pub enum IoTarget {
    Name(SmolStr),
    Reference(ValueRef),
}

use trust_runtime_core::io_image::IoEnumBinding;

#[derive(Debug, Clone, Default)]
struct IoBindingCodec {
    // Source-free bindings retain the exact alias/enum codec admitted by core;
    // artifact type-table indexes are never cast to authoring HIR identities.
    prepared: Option<trust_runtime_core::io_image::IoValueCodec>,
    wire_type: Option<TypeId>,
    enum_type: Option<IoEnumBinding>,
}

#[derive(Debug, Clone)]
pub struct IoBinding {
    pub target: IoTarget,
    pub address: IoAddress,
    pub value_type: Option<TypeId>,
    pub display_name: Option<SmolStr>,
    pub source: Option<SmolStr>,
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
    pub value_type: Option<TypeId>,
    pub value_type_name: Option<SmolStr>,
    pub value: IoSnapshotValue,
    pub source: Option<SmolStr>,
}

#[derive(Debug, Clone, Default)]
pub struct IoSnapshot {
    pub scan: Option<u64>,
    pub forced: Vec<IoAddress>,
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

/// The portable snapshot of this host snapshot (specification 34, ARCH-11).
///
/// The declared type becomes the IEC type name the host's JSON already shows: the enum's declared
/// name when there is one, otherwise the scalar wire type's name. A `trust_hir::TypeId` never
/// reaches the portable snapshot, so a target without the compiler's type table can hold it.
impl From<&IoSnapshot> for trust_runtime_services::live_values::io_snapshot::IoSnapshot {
    fn from(snapshot: &IoSnapshot) -> Self {
        Self {
            scan: snapshot.scan,
            forced: snapshot.forced.clone(),
            inputs: snapshot.inputs.iter().map(portable_entry).collect(),
            outputs: snapshot.outputs.iter().map(portable_entry).collect(),
            memory: snapshot.memory.iter().map(portable_entry).collect(),
        }
    }
}

fn portable_entry(
    entry: &IoSnapshotEntry,
) -> trust_runtime_services::live_values::io_snapshot::IoSnapshotEntry {
    use trust_runtime_services::live_values::io_snapshot::{
        IoSnapshotEntry as PortableEntry, IoSnapshotValue as PortableValue,
    };
    PortableEntry {
        name: entry.name.clone(),
        address: entry.address.clone(),
        value_type_name: entry.value_type_name.clone().or_else(|| {
            entry
                .value_type
                .and_then(io_value_type_name)
                .map(SmolStr::new)
        }),
        value: match &entry.value {
            IoSnapshotValue::Value(value) => PortableValue::Value(value.clone()),
            IoSnapshotValue::Error(error) => PortableValue::Error(error.clone()),
            IoSnapshotValue::Unresolved => PortableValue::Unresolved,
        },
        source: entry.source.clone(),
    }
}

#[cfg(test)]
mod portable_snapshot_tests {
    use super::*;
    use trust_runtime_core::stdlib::conversions::ConversionType;
    use trust_runtime_services::live_values::io_snapshot;

    /// Every I/O value type's portable name is the name the host's `TypeId` gives it today. A
    /// wrong entry in either name table (for example `TIME_OF_DAY` for `TOD`) fails here.
    #[test]
    fn host_type_id_names_match_the_portable_snapshot_names_for_every_io_type() {
        for (type_id, name) in [
            (TypeId::BOOL, "BOOL"),
            (TypeId::SINT, "SINT"),
            (TypeId::INT, "INT"),
            (TypeId::DINT, "DINT"),
            (TypeId::LINT, "LINT"),
            (TypeId::USINT, "USINT"),
            (TypeId::UINT, "UINT"),
            (TypeId::UDINT, "UDINT"),
            (TypeId::ULINT, "ULINT"),
            (TypeId::REAL, "REAL"),
            (TypeId::LREAL, "LREAL"),
            (TypeId::BYTE, "BYTE"),
            (TypeId::WORD, "WORD"),
            (TypeId::DWORD, "DWORD"),
            (TypeId::LWORD, "LWORD"),
            (TypeId::TIME, "TIME"),
            (TypeId::LTIME, "LTIME"),
            (TypeId::LDATE, "LDATE"),
            (TypeId::LTOD, "LTOD"),
            (TypeId::LDT, "LDT"),
            (TypeId::STRING, "STRING"),
            (TypeId::CHAR, "CHAR"),
            (TypeId::WCHAR, "WCHAR"),
        ] {
            assert_eq!(io_value_type_name(type_id), Some(name), "{name}: host name");
            let conversion =
                ConversionType::from_builtin_name(type_id.builtin_name().expect("builtin name"))
                    .expect("builtin conversion type");
            assert_eq!(
                io_snapshot::value_type_name(conversion),
                Some(name),
                "{name}: portable name"
            );
        }
    }

    #[test]
    fn portable_conversion_names_scalar_and_enum_types() {
        let snapshot = IoSnapshot {
            scan: Some(3),
            forced: vec![IoAddress::parse("%QX0.0").expect("forced address")],
            inputs: Vec::new(),
            outputs: Vec::new(),
            memory: vec![
                IoSnapshotEntry {
                    name: Some(SmolStr::new("Speed")),
                    address: IoAddress::parse("%MD0").expect("address"),
                    value_type: Some(TypeId::REAL),
                    value_type_name: None,
                    value: IoSnapshotValue::Value(Value::Real(1.5)),
                    source: None,
                },
                IoSnapshotEntry {
                    name: Some(SmolStr::new("State")),
                    address: IoAddress::parse("%QD4").expect("address"),
                    value_type: Some(TypeId::DINT),
                    value_type_name: Some(SmolStr::new("TrafficState")),
                    value: IoSnapshotValue::Value(Value::DInt(1)),
                    source: None,
                },
            ],
        };

        let portable = io_snapshot::IoSnapshot::from(&snapshot);
        assert_eq!(portable.scan, Some(3));
        assert_eq!(
            portable.forced,
            vec![IoAddress::parse("%QX0.0").expect("forced address")]
        );
        assert_eq!(portable.memory[0].value_type_name.as_deref(), Some("REAL"));
        assert_eq!(
            portable.memory[1].value_type_name.as_deref(),
            Some("TrafficState")
        );
    }
}
