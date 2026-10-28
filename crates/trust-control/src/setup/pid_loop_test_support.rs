//! The fixtures that the PID loop generator tests and the PID record tests share (specification 35, C0, C1 Setup): the record field of a
//! parameter's symbol and the seven retained states of the `Control.PID` record. Declared under `cfg(test)` by `pid_loop.rs`.

pub(super) const STATE_FIELDS: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

/// The last segment of a parameter's path: the record field.
pub(super) fn field(symbol: &str) -> String {
    symbol.rsplit('.').next().unwrap_or_default().to_string()
}
