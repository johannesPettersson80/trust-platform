/// I/O driver interface for process image exchange.
pub trait IoDriver: Send {
    /// Read hardware or simulated inputs into the input image.
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError>;

    /// Write the output image to hardware or a simulator.
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError>;

    /// True only for an in-process simulator with no physical I/O side effects.
    fn is_simulated(&self) -> bool { false }

    /// Report the current driver health.
    fn health(&self) -> IoDriverHealth {
        IoDriverHealth::Ok
    }

    /// The bytes of the input image it fills, from its own configuration, so a value located
    /// there turns stale while its link exchanges no data (specification 42, §5 item 4); `0..
    /// usize::MAX` for a driver that fills the image from its start. None marks nothing.
    fn input_bytes(&self) -> Vec<std::ops::Range<usize>> {
        Vec::new()
    }

    /// The protocol's own state in words, beside the health (specification 42, §4 item 9).
    fn detail(&self) -> Option<SmolStr> {
        None
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
    fn is_simulated(&self) -> bool { true }
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Byte ranges sorted, with overlapping and touching ones joined (specification 42, §5 item 4).
pub(crate) fn joined_bytes(mut ranges: Vec<std::ops::Range<usize>>) -> Vec<std::ops::Range<usize>> {
    ranges.retain(|range| range.start < range.end);
    ranges.sort_by_key(|range| range.start);
    let mut joined: Vec<std::ops::Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match joined.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => joined.push(range),
        }
    }
    joined
}
