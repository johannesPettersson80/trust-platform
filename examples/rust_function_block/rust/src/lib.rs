//! The Rust of the example `rust_function_block`: one function block, which `src/Main.st` calls.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// A setpoint ramp: moves `out` towards `target` by at most `rate` per second. `done` tells whether
/// `out` has reached `target`.
#[trust_fb(name = "RAMP")]
#[budget("2us")]
pub struct Ramp {
    #[input]
    pub target: f64,
    #[input]
    #[init(10.0)]
    pub rate: f64,
    #[output]
    pub out: f64,
    #[output]
    pub done: bool,
}

impl Body for Ramp {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        // The most `out` may move in this call: the rate times the task's interval in seconds. A
        // negative rate moves as fast as its size says.
        let mut step = self.rate * cx.interval().as_nanos() as f64 / 1e9;
        if step < 0.0 {
            step = -step;
        }
        let gap = self.target - self.out;
        if gap > step {
            self.out += step;
        } else if gap < -step {
            self.out -= step;
        } else {
            self.out = self.target;
        }
        self.done = self.out == self.target;
        Ok(())
    }
}
