pub use trust_runtime_core::io_address::{IoAddress, IoSize};

#[derive(Debug, Clone)]
pub enum IoTarget {
    Name(SmolStr),
    Reference(ValueRef),
}

#[derive(Debug, Clone)]
struct IoEnumBinding {
    type_name: SmolStr,
    variants: Vec<(SmolStr, i64)>,
}

#[derive(Debug, Clone, Default)]
struct IoBindingCodec {
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
