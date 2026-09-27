//! Reviewed project-owned abort input; the PLC retains actuator-policy ownership.
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use trust_hir::TypeId;

use super::values::{snapshot_type, snapshot_value};
use crate::debug::DebugControl;
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot};
use crate::memory::IoArea;
use crate::value::Value;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AbortBindingParams {
    address: String,
    value_type: String,
    asserted_value: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssertionState {
    NotRequested,
    Pending,
    Observed,
    Failed,
}

#[derive(Clone, Debug)]
pub(super) struct AbortBinding {
    address: IoAddress,
    address_text: String,
    asserted_value: bool,
    write_value: Value,
    state: AssertionState,
    observed_scan: Option<u64>,
}

impl AbortBinding {
    pub(super) fn decode(
        params: AbortBindingParams,
        kind: &str,
        activation: &IoAddress,
        activation_type: Option<TypeId>,
        snapshot: &IoSnapshot,
    ) -> Result<Self, (String, &'static str)> {
        if kind != "project_sequence" {
            return Err((
                "abort bindings require a project-owned sequence".into(),
                "invalid_experiment",
            ));
        }
        if !params.value_type.trim().eq_ignore_ascii_case("BOOL")
            || activation_type != Some(TypeId::BOOL)
        {
            return Err((
                "activation and abort require declared BOOL bindings".into(),
                "incompatible_binding",
            ));
        }
        let address =
            IoAddress::parse(&params.address).map_err(|e| (e.to_string(), "invalid_address"))?;
        validate_process_image_address(&address).map_err(|e| (e.to_string(), "invalid_address"))?;
        if address == *activation
            || address.area == IoArea::Input
            || activation.area == IoArea::Input
        {
            return Err((
                "activation and abort must be distinct writable bindings".into(),
                "incompatible_binding",
            ));
        }
        if snapshot.forced.contains(&address) {
            return Err(("project abort signal is forced".into(), "forced_signal"));
        }
        if snapshot_type(snapshot, &address) != Some(TypeId::BOOL) {
            return Err((
                "project abort type does not match its compiled BOOL binding".into(),
                "incompatible_binding",
            ));
        }
        let Some(Value::Bool(current)) = snapshot_value(snapshot, &address) else {
            return Err((
                "project abort value is unavailable".into(),
                "incompatible_binding",
            ));
        };
        if *current == params.asserted_value {
            return Err((
                "project abort is already asserted; explicitly recover before starting".into(),
                "abort_already_asserted",
            ));
        }
        let write_value = crate::io::coerce_to_io(
            Value::Bool(params.asserted_value),
            TypeId::BOOL,
            address.size,
        )
        .map_err(|e| (e.to_string(), "incompatible_binding"))?;
        Ok(Self {
            address,
            address_text: params.address,
            asserted_value: params.asserted_value,
            write_value,
            state: AssertionState::NotRequested,
            observed_scan: None,
        })
    }

    pub(super) fn asserted(&self, snapshot: &IoSnapshot) -> Result<bool, &'static str> {
        if snapshot.forced.contains(&self.address) {
            return Err("abort_signal_forced");
        }
        if snapshot_type(snapshot, &self.address) != Some(TypeId::BOOL) {
            return Err("bad_abort_binding");
        }
        match snapshot_value(snapshot, &self.address) {
            Some(Value::Bool(value)) => Ok(*value == self.asserted_value),
            _ => Err("bad_abort_binding"),
        }
    }

    pub(super) fn request(&mut self, debug: &DebugControl) {
        debug.enqueue_io_write(self.address.clone(), self.write_value.clone());
        self.state = AssertionState::Pending;
        self.observed_scan = None;
    }

    pub(super) fn confirm(&mut self, snapshot: &IoSnapshot) -> bool {
        let expected_asserted = self.state != AssertionState::NotRequested;
        if self.asserted(snapshot) != Ok(expected_asserted) {
            return false;
        }
        if expected_asserted {
            self.state = AssertionState::Observed;
            self.observed_scan = snapshot.scan;
        }
        true
    }

    pub(super) fn fail_confirmation(&mut self) {
        if self.state == AssertionState::Pending {
            self.state = AssertionState::Failed;
        }
    }

    pub(super) fn json(&self) -> JsonValue {
        json!({"address":self.address_text,"asserted_value":self.asserted_value,
            "state":match self.state { AssertionState::NotRequested=>"not_requested",AssertionState::Pending=>"pending",AssertionState::Observed=>"observed",AssertionState::Failed=>"failed" },
            "observed_scan":self.observed_scan})
    }
}
