//! Loopback I/O driver for development.

use crate::error::RuntimeError;
use crate::io::IoDriver;

#[derive(Debug, Default)]
pub struct LoopbackIoDriver {
    last_outputs: Vec<u8>,
}

impl IoDriver for LoopbackIoDriver {
    fn is_simulated(&self) -> bool {
        true
    }
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let len = inputs.len().min(self.last_outputs.len());
        inputs[..len].copy_from_slice(&self.last_outputs[..len]);
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.last_outputs.clear();
        self.last_outputs.extend_from_slice(outputs);
        Ok(())
    }
}
