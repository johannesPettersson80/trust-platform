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

    /// The input image from its start, which it fills with the last output image (specification
    /// 42, §5 item 4).
    fn input_bytes(&self) -> Vec<std::ops::Range<usize>> {
        std::iter::once(0..usize::MAX).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loopback_declares_the_input_image_from_its_start() {
        // Specification 42, §5 item 4: the loopback driver copies the last output image over the
        // input image from its first byte, so it declares the image from its start, as a raw
        // topic does, and a driver composed after it overwrites what it copied. Catches a loopback
        // that declares nothing, which no overlap warning would name.
        assert_eq!(
            LoopbackIoDriver::default().input_bytes(),
            vec![0..usize::MAX]
        );
    }
}
