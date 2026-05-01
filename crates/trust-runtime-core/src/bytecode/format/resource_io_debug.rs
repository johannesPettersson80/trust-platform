#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceMeta {
    pub resources: Vec<ResourceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry {
    pub name_idx: u32,
    pub inputs_size: u32,
    pub outputs_size: u32,
    pub memory_size: u32,
    pub tasks: Vec<TaskEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEntry {
    pub name_idx: u32,
    pub priority: u32,
    pub interval_nanos: i64,
    pub single_name_idx: Option<u32>,
    pub program_name_idx: Vec<u32>,
    pub fb_ref_idx: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IoMap {
    pub bindings: Vec<IoBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoBinding {
    pub address_str_idx: u32,
    pub ref_idx: u32,
    pub type_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugMap {
    pub entries: Vec<DebugEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEntry {
    pub pou_id: u32,
    pub code_offset: u32,
    pub file_idx: u32,
    pub line: u32,
    pub column: u32,
    pub kind: u8,
}
