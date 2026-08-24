#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoSize {
    Bit,
    Byte,
    Word,
    DWord,
    LWord,
    Bytes(u32),
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
        if rest.is_empty() {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        if rest == "*" {
            return Ok(Self {
                area,
                size: IoSize::Bit,
                byte: 0,
                bit: 0,
                path: Vec::new(),
                wildcard: true,
            });
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
            ch if ch.is_ascii_digit() => (IoSize::Bit, rest.as_str()),
            _ => return Err(RuntimeError::InvalidIoAddress(trimmed.into())),
        };

        let mut path: Vec<u32> = Vec::new();
        let mut bit = 0u8;
        let parts: Vec<&str> = rest.split('.').collect();
        if matches!(size, IoSize::Bit) && parts.len() >= 2 {
            for part in &parts[..parts.len() - 1] {
                path.push(parse_u32(part, trimmed)?);
            }
            let bit_part = parts[parts.len() - 1];
            bit = parse_u8(bit_part, trimmed)?;
            if bit > 7 {
                return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
            }
        } else {
            for part in &parts {
                path.push(parse_u32(part, trimmed)?);
            }
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
fn parse_u32(value: &str, full: &str) -> Result<u32, RuntimeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RuntimeError::InvalidIoAddress(full.into()));
    }
    value
        .parse::<u32>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}

fn parse_u8(value: &str, full: &str) -> Result<u8, RuntimeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RuntimeError::InvalidIoAddress(full.into()));
    }
    value
        .parse::<u8>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}
