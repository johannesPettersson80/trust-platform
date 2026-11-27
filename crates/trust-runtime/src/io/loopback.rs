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

    /// The input image from its start as far as the output image reaches, which it copies over
    /// the inputs (specification 42, §5 item 4); nothing without an output image.
    fn input_bytes_with_outputs(&self, outputs: usize) -> Vec<std::ops::Range<usize>> {
        crate::io::declared_inputs::loopback(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loopback_declares_the_input_bytes_its_copy_of_the_outputs_reaches() {
        // Specification 42, §5 item 4: the loopback driver copies the last output image over the
        // input image from its first byte, as far as both reach, so with a 1-byte output image
        // it fills input byte 0 of 3 and declares 0..1, and with no output image nothing. Catches
        // a loopback that declares nothing, which neither a stale value nor an overlap warning
        // would name, and one that declares input bytes its copy never reaches.
        let mut loopback = LoopbackIoDriver::default();
        loopback.write_outputs(&[0xA5]).unwrap();
        let mut inputs = [0u8; 3];
        loopback.read_inputs(&mut inputs).unwrap();
        assert_eq!(inputs, [0xA5, 0, 0], "the copy reaches byte 0 alone");
        assert_eq!(loopback.input_bytes_with_outputs(1), vec![0..1]);
        assert_eq!(
            loopback.input_bytes_with_outputs(0),
            Vec::<std::ops::Range<usize>>::new()
        );
    }
}
