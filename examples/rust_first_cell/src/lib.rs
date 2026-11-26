//! The press cell (specification 48 §14 and §15): a press that joins two parts in a fixture, written
//! in Rust. `trust.toml` runs its program as the instance `Cell` every 10 ms and binds its inputs and
//! outputs to the fixture's button, sensors, valves and lamp; `simulation.toml` simulates the clamp
//! and the press. When you save this file, truST writes the Structured Text that declares the
//! program, and the configuration of `trust.toml`, into `src/generated/trust/`.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// How long the clamp may take to close or to open, in milliseconds.
pub const CLAMP_TIME_MS: i64 = 1_000;
/// How long the press may take to go down or up, in milliseconds.
pub const PRESS_TIME_MS: i64 = 2_000;
/// How long the press holds the parts together, in milliseconds.
pub const HOLD_TIME_MS: i64 = 500;

/// The press cell. When a part lies in the fixture and the operator presses Start, it clamps the
/// part, presses, holds, lifts the press, opens the clamp and counts the part; then it waits until
/// the part is taken out. A clamp or a press that does not reach its sensor in time faults the
/// program, naming the wait.
#[trust_program(name = "PRESS_CELL", sequence = "run")]
#[budget("50us")]
pub struct PressCell {
    /// The operator's Start button.
    #[input]
    pub start: bool,
    /// The fixture's part sensor.
    #[input]
    pub part_present: bool,
    /// The clamp's sensor: the clamp is closed.
    #[input]
    pub clamp_closed: bool,
    /// The press's sensor: the press is down.
    #[input]
    pub press_down: bool,
    /// The clamp's valve.
    #[output]
    pub clamp: bool,
    /// The press's valve.
    #[output]
    pub press: bool,
    /// The Ready lamp: the cell waits for a part and Start.
    #[output]
    pub ready: bool,
    /// The step the sequence is in, for Live Values and a screen.
    #[output]
    pub active_step: Str<16>,
    /// The parts pressed since the program started.
    #[output]
    pub parts: u32,
}

#[trust_sequence]
impl PressCell {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.active_step, "READY");
            self.ready = true;
            s.until(|me: &Self| me.part_present && me.start).await;
            self.ready = false;

            s.step(&mut self.active_step, "CLAMP");
            self.clamp = true;
            s.until(|me: &Self| me.clamp_closed)
                .within(Time::from_millis(s, CLAMP_TIME_MS)?)
                .await?;

            s.step(&mut self.active_step, "PRESS");
            self.press = true;
            s.until(|me: &Self| me.press_down)
                .within(Time::from_millis(s, PRESS_TIME_MS)?)
                .await?;

            s.step(&mut self.active_step, "HOLD");
            s.wait(Time::from_millis(s, HOLD_TIME_MS)?).await;

            s.step(&mut self.active_step, "LIFT");
            self.press = false;
            s.until(|me: &Self| !me.press_down)
                .within(Time::from_millis(s, PRESS_TIME_MS)?)
                .await?;

            s.step(&mut self.active_step, "UNCLAMP");
            self.clamp = false;
            s.until(|me: &Self| !me.clamp_closed)
                .within(Time::from_millis(s, CLAMP_TIME_MS)?)
                .await?;
            self.parts = self.parts.wrapping_add(1);

            s.step(&mut self.active_step, "UNLOAD");
            s.until(|me: &Self| !me.part_present).await;
        }
    }
}
