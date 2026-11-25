//! The room heating's commissioning trace (specification 48 RU-59 and RU-60). The file
//! `traces/commissioning.trusttrace` is 30 s of this program and its simulated plant on a computer
//! runtime, recorded with `trust-runtime ctl record --duration 30s`, with Start pressed 1.21 s in as
//! the Heating panel's button does. In the recording, the pump runs from 1.33 s, FS-101 sees flow at
//! 5.33 s, the heater runs from 5.53 s and the fan from 19.33 s, once the tank reaches 40 °C.
//!
//! The replay gives the program as it is now the recorded tasks, inputs, changes from outside and
//! clock readings, cycle by cycle, and compares its outputs with the trace's. The test catches a
//! change that alters what the program decides, such as a heater that starts later, at the first
//! cycle that differs, and a platform that computes other results from the same inputs
//! (specification 44 NX-04): CI replays the trace recorded on Linux on macOS and Windows too. A
//! change made on purpose records the trace again (the example's README, "Machine tests").

#[test]
fn the_commissioning_trace_replays_with_the_same_outputs() -> trust_sim::Result<()> {
    let project = trust_sim::Project::open("..")?;
    trust_sim::Replay::open("../traces/commissioning.trusttrace")?.run(&project)
}
