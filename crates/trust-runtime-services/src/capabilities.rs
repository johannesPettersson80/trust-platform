//! The `capabilities` operation (specification 11, the runtime device capabilities contract;
//! decision D-C4-1): the device descriptor admission reads (specification 34 §5.1.1, COMPAT-02)
//! and, on a device without an operating system, the reset its boot read (specification 34, the
//! fixed-size reset record).
//!
//! The reply is rendered in exactly one place, here, so a computer and a device without an
//! operating system answer the same shape: the platform supplies the descriptor through
//! [`StatusPort::device_descriptor`] and its last reset through [`StatusPort::last_reset`].

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Map, Value};
use smol_str::SmolStr;
use trust_runtime_core::deployment::{Capabilities, DeviceDescriptor, RequestedLimits};

use crate::ports::StatusPort;
use crate::protocol::ControlResponse;

/// `capabilities`: the device's descriptor and, on a device without an operating system, the
/// reset its boot read. It takes no parameters and changes nothing (specification 11, the runtime
/// device capabilities contract).
#[must_use]
pub fn capabilities<H: StatusPort>(id: u64, host: &H) -> ControlResponse {
    let descriptor = host.device_descriptor();
    let mut result = descriptor_json(&descriptor, host.last_reset().as_ref());
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "firmware".into(),
            host.firmware().map_or(Value::Null, |facts| facts.to_json()),
        );
    }
    ControlResponse::ok(id, result)
}

/// The `capabilities` result: every descriptor field under its own name, and `last_reset`, which
/// is null on a computer (specification 11, the runtime device capabilities contract).
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn descriptor_json(descriptor: &DeviceDescriptor, last_reset: Option<&LastReset>) -> Value {
    json!({
        "program_versions": program_versions_json(descriptor),
        "scheduling_profiles": descriptor
            .scheduling_profiles
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "numeric_contracts": descriptor
            .numeric_contracts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "timing_evidence": descriptor.timing_evidence.to_string(),
        "capabilities": capabilities_json(descriptor.capabilities),
        "maxima": maxima_json(&descriptor.maxima),
        "event_store_default": descriptor.event_store_default,
        "event_store_max": descriptor.event_store_max,
        "application_slot_bytes": descriptor.application_slot_bytes,
        "program_memory_bytes": descriptor.program_memory_bytes,
        "last_reset": last_reset.map(last_reset_json),
    })
}

/// The program formats the device runs, each `<major>.<minor>` (specification 44 §10.1).
fn program_versions_json(descriptor: &DeviceDescriptor) -> Value {
    Value::Array(
        descriptor
            .program_versions
            .iter()
            .map(|version| Value::String(format!("{}.{}", version.major, version.minor)))
            .collect(),
    )
}

/// The capabilities the device supplies, under their wire names.
fn capabilities_json(capabilities: Capabilities) -> Value {
    let mut names = Vec::new();
    for bit in 0..16_u16 {
        if capabilities.0 & (1_u16 << bit) != 0 {
            names.push(Value::String(capability_name(bit)));
        }
    }
    Value::Array(names)
}

/// The wire name of capability bit `bit`; bit 0 is the platform wall clock (specification 34
/// §5.2), and a bit the specification has not named is reported by its number.
fn capability_name(bit: u16) -> String {
    match bit {
        0 => String::from("wall_clock"),
        other => format!("capability_bit_{other}"),
    }
}

/// The device's maximum for each preparation limit under its `PreparationLimits` field name: a
/// number when the device bounds the limit, `null` when it has no maximum for it.
fn maxima_json(maxima: &RequestedLimits) -> Value {
    let mut object = Map::new();
    for (limit, value) in maxima.values() {
        object.insert(String::from(limit.name()), maximum_json(value));
    }
    Value::Object(object)
}

/// One maximum as JSON. A limit at or above `usize::MAX`, the largest value the platform holds, is
/// no maximum: the descriptor writes "no maximum" as `u64::MAX` on a chip and as `usize::MAX` on a
/// computer. It renders as `null`, because a JSON number that large is rounded by a JavaScript
/// reader (specification 11, the runtime device capabilities contract).
fn maximum_json(value: u64) -> Value {
    if value >= platform_largest() {
        Value::Null
    } else {
        Value::from(value)
    }
}

/// The largest value the platform holds, as a `u64`.
fn platform_largest() -> u64 {
    u64::try_from(usize::MAX).unwrap_or(u64::MAX)
}

/// The last reset the device's boot read, by its cause, text and boot counter.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn last_reset_json(last_reset: &LastReset) -> Value {
    json!({
        "cause": last_reset.cause.as_str(),
        "text": last_reset.text.as_str(),
        "boot_count": last_reset.boot_count,
    })
}

/// The cause of the previous boot's reset, by its specification name (specification 11, the
/// runtime device capabilities contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetCauseName {
    /// The boot read no record and no watchdog reason: it started afresh, after a power-on or a
    /// host reset (or a reason the hardware cannot name).
    PowerOn,
    /// The firmware's panic path ended the previous boot.
    Panic,
    /// The hardware watchdog reset the chip.
    Watchdog,
    /// A requested restart ended the previous boot.
    Requested,
}

impl ResetCauseName {
    /// The cause's wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PowerOn => "power_on",
            Self::Panic => "panic",
            Self::Watchdog => "watchdog",
            Self::Requested => "requested",
        }
    }
}

/// The last reset a device without an operating system read from its reset record at boot
/// (specification 11, the runtime device capabilities contract). A computer has none: it starts
/// as a new process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastReset {
    /// Why the previous boot ended.
    pub cause: ResetCauseName,
    /// The record's text, for example a panic location, empty when the boot read no record.
    pub text: SmolStr,
    /// The boot counter the chip keeps, counted for this boot, reported for every reset whether or
    /// not a record exists.
    pub boot_count: u32,
}

#[cfg(test)]
mod contract_tests;
