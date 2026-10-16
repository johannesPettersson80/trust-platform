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

    /// Report the output-path health that a safe-state handoff depends on (specification 11,
    /// safe-state handoff confirmation). Defaults to `health()`; a driver overrides it only when
    /// its reported health includes conditions that concern received data alone.
    fn output_health(&self) -> IoDriverHealth {
        self.health()
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
