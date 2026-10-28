//! The commissioning profile of a controller structure (specification 35, C0 and E6.3, E6.4; `commissioningProfile.ts`): its
//! label, its references, the tracking that the structure requires of the runtime, and the controller semantics that the
//! commissioning lifecycle plugs in. The `TypeScript` record holds closures over the loop, the model and the declared states.
//! Here the record holds the data, and `ProfileKind` dispatches each behaviour (binding, states, tracking, targets) to the
//! structure's functions. A new structure adds a `ProfileKind` variant.

// The record's name, `CommissioningProfile`, ends with the module's name. It is the TypeScript name, which the lifecycle uses.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use super::declared_initialization::{declared_states, DeclaredStateProfile};
use super::transfer::{TransferBinding, TransferValue};
use super::transfer_tracking::TransferTracking;
use crate::failure::Failure;
use crate::validation::engineer_verification_plan::EngineerRequirements;

/// A tracking feature that a profile requires of the runtime (`requiredTracking`, annex E6.3 `supported_tracking`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredTracking {
    /// `declared-initialization-formulas`: the runtime reads the fresh sources of the declared states at the boundary.
    DeclaredInitializationFormulas,
}

impl RequiredTracking {
    /// The word that the runtime reports for this feature (E6.3).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::DeclaredInitializationFormulas => "declared-initialization-formulas",
        }
    }
}

/// The structure that a profile describes, with the data its behaviour needs.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileKind {
    /// A `declared-state-v1` structure: a library `Control.PID`, or another controller whose states are declared (C0).
    DeclaredState(DeclaredStateProfile),
}

/// A commissioning profile (`CommissioningProfile`): the structure's label, its references, the tracking it requires, and its
/// kind, which dispatches the binding, the states, the tracking and the targets.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissioningProfile {
    /// The structure's label, as the user reads it.
    pub label: String,
    /// The reference symbols of the loop (`references`).
    pub references: Vec<String>,
    /// The tracking that the structure requires of the runtime (`requiredTracking`).
    pub required_tracking: Vec<RequiredTracking>,
    /// The structure and its data.
    pub kind: ProfileKind,
}

impl CommissioningProfile {
    /// The tracking kind word that the structure sends (`trackingKind`, E6.4).
    #[must_use]
    pub fn tracking_kind(&self) -> &'static str {
        match &self.kind {
            ProfileKind::DeclaredState(_) => "declared-state-v1",
        }
    }

    /// The reference targets for a reviewed task (`targets`, B6.2).
    ///
    /// # Errors
    /// The structure's targets do not fit the loop and the model (see the structure's own function).
    pub fn targets(&self, task: &EngineerRequirements) -> Result<BTreeMap<String, f64>, Failure> {
        match &self.kind {
            ProfileKind::DeclaredState(profile) => profile.targets(task),
        }
    }

    /// The binding that the structure's transfer writes (`binding`, A6.3).
    ///
    /// # Errors
    /// The structure's binding does not fit the loop (see the structure's own function).
    pub fn binding(&self) -> Result<TransferBinding, Failure> {
        match &self.kind {
            ProfileKind::DeclaredState(profile) => profile.binding(),
        }
    }

    /// The states that the runtime's observation reports, rounded as they are typed (`states`, E6.3).
    ///
    /// # Errors
    /// A state of the binding has no reported value, or a value does not fit its member.
    pub fn states(
        &self,
        binding: &TransferBinding,
        observed: &BTreeMap<String, TransferValue>,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        match &self.kind {
            ProfileKind::DeclaredState(_) => declared_states(binding, observed),
        }
    }

    /// The tracking that the transfer sends (`tracking`, E6.4): an Auto entry aligns the output (`initialize`); a target change
    /// does not.
    ///
    /// # Errors
    /// The structure's tracking does not fit the binding and the loop (see the structure's own function).
    pub fn tracking(
        &self,
        binding: &TransferBinding,
        initialize: bool,
    ) -> Result<TransferTracking, Failure> {
        match &self.kind {
            ProfileKind::DeclaredState(profile) => profile.tracking(binding, initialize),
        }
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
