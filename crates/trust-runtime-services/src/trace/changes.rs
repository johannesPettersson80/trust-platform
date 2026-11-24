//! The changes from outside that Live Values applies, recorded as a cycle trace keeps them
//! (specification 48 RU-59 and Appendix C, C.6): a variable's write and force by the path the
//! debugger names it by, a member of an instance included, and an address's one-shot write and
//! force by the bytes they give the process image. A force is recorded once, in the cycle it is set
//! or released before; the trace's start state holds the forces in effect at its first cycle.

use alloc::vec::Vec;

use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea as ImageArea;
use trust_runtime_core::native::memory::member_display_path;
use trust_runtime_core::native::variables::{image_value_bytes, CompiledVariables};
use trust_runtime_core::trace::{ChangeRef, ChangeSource, IoArea};
use trust_runtime_core::value::Value;

use super::Recorder;
use crate::live_values::apply::ForceEffect;
use crate::live_values::queues::{ForcedVar, ForcedVarTarget, PendingVarTarget};
use crate::method_call::MethodCall;

impl Recorder {
    /// A queued write of `target` that `variables` has applied, recorded by the target's path as
    /// [`Self::written`] records it. A target whose path cannot be named ends the recording
    /// `memory_full` at the cycle's end.
    pub fn written_target(
        &mut self,
        source: ChangeSource,
        target: &PendingVarTarget,
        variables: &dyn CompiledVariables,
    ) {
        if !self.cycle.open {
            return;
        }
        let path = match target {
            PendingVarTarget::Global(path) | PendingVarTarget::Retain(path) => Some(path.as_str()),
            PendingVarTarget::Instance(_, instance, member) => {
                member_display_path(&variables.program_image(), instance.0, member)
            }
            PendingVarTarget::Local(..) => None,
        };
        match path {
            Some(path) => self.written(source, path, variables),
            None => self.cycle.overflow = true,
        }
    }

    /// A one-shot write of `value` at `address` (`io.write`) that the boundary applied, recorded as
    /// the bytes it gives the process image.
    pub fn address_written(
        &mut self,
        source: ChangeSource,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
    ) {
        if !self.cycle.open {
            return;
        }
        self.with_address_bytes(address, value, variables, |recorder, bytes| {
            recorder.change(
                source,
                &ChangeRef::AddressWrite {
                    area: trace_area(address.area),
                    bit: trace_bit(address),
                    offset: address.byte,
                    bytes,
                },
            );
        });
    }

    /// A force set or released since the last cycle start ([`ForceEffect`]), recorded once, in the
    /// cycle it takes effect before. The trace's first cycle records none: its start state holds
    /// the forces then in effect.
    pub fn forced(
        &mut self,
        source: ChangeSource,
        effect: ForceEffect<'_>,
        variables: &dyn CompiledVariables,
    ) {
        if !self.cycle.open || self.cycles() == 0 {
            return;
        }
        match effect {
            ForceEffect::VariableSet(target) => match forced_path(target, variables) {
                Some(path) => self.variable_force(source, path, variables, false),
                None => self.cycle.overflow = true,
            },
            ForceEffect::VariableReleased(target) => match forced_path(target, variables) {
                Some(path) => self.change(source, &ChangeRef::VariableRelease { path }),
                None => self.cycle.overflow = true,
            },
            ForceEffect::AddressSet(address, value) => {
                self.with_address_bytes(address, value, variables, |recorder, bytes| {
                    recorder.change(
                        source,
                        &ChangeRef::AddressForce {
                            area: trace_area(address.area),
                            bit: trace_bit(address),
                            offset: address.byte,
                            bytes,
                        },
                    );
                });
            }
            ForceEffect::AddressReleased(address) => self.change(
                source,
                &ChangeRef::AddressRelease {
                    area: trace_area(address.area),
                    bit: trace_bit(address),
                    offset: address.byte,
                    size: address_size(address.size),
                },
            ),
        }
    }

    /// A method call that a tool made between two scans (`method.call`), recorded as the next
    /// cycle's change: the instance by the path the debugger names it by (`instance`), the
    /// interface, the method, and each input's name with its literal as the request gave it.
    pub fn method_called(&mut self, source: ChangeSource, instance: &str, call: &MethodCall) {
        let inputs: Vec<(&str, &str)> = call
            .inputs
            .iter()
            .zip(&call.literals)
            .map(|((name, _), literal)| (name.as_str(), literal.as_str()))
            .collect();
        self.change(
            source,
            &ChangeRef::MethodCall {
                instance,
                interface: &call.interface,
                method: &call.method,
                inputs: &inputs,
            },
        );
    }

    /// Force changes were lost since the last cycle start: the recording ends `memory_full` at the
    /// cycle's end.
    pub fn force_changes_lost(&mut self) {
        if self.cycle.open {
            self.cycle.overflow = true;
        }
    }

    /// The forces in effect at the start, on the trace's first cycle, once the boundary applied
    /// them: each variable force as the bytes its variable holds, and each address force as the
    /// bytes it gives the process image.
    pub fn start_forces(
        &mut self,
        source: ChangeSource,
        vars: &[ForcedVar],
        io: &[(IoAddress, Value)],
        variables: &dyn CompiledVariables,
    ) {
        for force in vars {
            match forced_path(&force.target, variables) {
                Some(path) => self.variable_force(source, path, variables, true),
                None => self.cycle.overflow = true,
            }
        }
        for (address, value) in io {
            self.with_address_bytes(address, value, variables, |recorder, bytes| {
                recorder.start_force(
                    source,
                    &ChangeRef::AddressForce {
                        area: trace_area(address.area),
                        bit: trace_bit(address),
                        offset: address.byte,
                        bytes,
                    },
                );
            });
        }
    }

    /// The force of the variable `path`, recorded as the bytes it holds: at the start (`start`), or
    /// as a change of the cycle.
    fn variable_force(
        &mut self,
        source: ChangeSource,
        path: &str,
        variables: &dyn CompiledVariables,
        start: bool,
    ) {
        // The room is taken out while its bytes are recorded and put back: moving a vector
        // allocates nothing.
        let mut room = core::mem::take(&mut self.value);
        match variables.read_named_bytes(path, &mut room) {
            Ok((len, type_name)) => {
                let force = ChangeRef::VariableForce {
                    path,
                    type_name: &type_name,
                    value: &room[..len],
                };
                if start {
                    self.start_force(source, &force);
                } else {
                    self.change(source, &force);
                }
            }
            Err(_) => self.cycle.overflow = true,
        }
        self.value = room;
    }

    /// Calls `record` with the bytes that `value` gives the process image at `address`, worked out
    /// in the reserved room for a value; a value that the address does not take, or that the room
    /// cannot hold, ends the recording `memory_full` at the cycle's end.
    fn with_address_bytes(
        &mut self,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
        record: impl FnOnce(&mut Self, &[u8]),
    ) {
        let mut room = core::mem::take(&mut self.value);
        match image_value_bytes(&variables.program_image(), address, value, &mut room) {
            Some(len) => record(self, &room[..len]),
            None => self.cycle.overflow = true,
        }
        self.value = room;
    }
}

/// The path the debugger names a forced variable by: a global's or a retained variable's name, and
/// a member of an instance by its symbol's display name.
fn forced_path<'v>(
    target: &'v ForcedVarTarget,
    variables: &'v dyn CompiledVariables,
) -> Option<&'v str> {
    match target {
        ForcedVarTarget::Global(path) | ForcedVarTarget::Retain(path) => Some(path.as_str()),
        ForcedVarTarget::Instance(_, instance, member) => {
            member_display_path(&variables.program_image(), instance.0, member)
        }
    }
}

/// The trace's area of a process image's area (Appendix C, C.6).
#[must_use]
pub fn trace_area(area: ImageArea) -> IoArea {
    match area {
        ImageArea::Input => IoArea::Input,
        ImageArea::Output => IoArea::Output,
        ImageArea::Memory => IoArea::Memory,
    }
}

/// The bit of a one-bit address, and none for whole bytes.
#[must_use]
pub fn trace_bit(address: &IoAddress) -> Option<u8> {
    matches!(address.size, IoSize::Bit).then_some(address.bit)
}

/// The bytes an address of `size` spans, one for a bit.
#[must_use]
pub fn address_size(size: IoSize) -> u32 {
    match size {
        IoSize::Bit | IoSize::Byte => 1,
        IoSize::Word => 2,
        IoSize::DWord => 4,
        IoSize::LWord => 8,
        IoSize::Bytes(count) => count,
    }
}
