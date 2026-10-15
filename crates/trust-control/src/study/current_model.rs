//! The study's current model (specification 35, B3.7, A8.1 and A8.4): the one model every stage reads. It is the latest of the
//! study's two models, the fit of its Identify result and the model the engineer supplied (`importModel`). `importModel` makes the
//! supplied model current, a later Identify makes its fit current, and Reset clears both: the study keeps the source of the model it
//! kept last ([`super::Study::model_source`]), and [`current_model`] reads both sections through it.
//!
//! A supplied model was not fitted from a recording, so it has no recording, no fit error and no Identify presentation (B3.7). Its
//! channel names are not matched against the loop's signals: a changed signal starts a new study (A8.2), and the study keeps the model
//! while it does.

#![allow(clippy::module_name_repetitions)] // The type names its concept: the current model.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;
use crate::model::{GeneralModel, ModelValidation};
use crate::study::general_model_persistence::SuppliedModelSection;
use crate::study::identify::IdentifySection;
use crate::study::Study;

/// Where the study's current model came from (A8.4): the fit of its Identify result, or the model the engineer supplied with
/// `importModel` (B3.7). The study keeps it as its marker, so the study says which of its two models is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CurrentModelSource {
    /// The fit of the study's Identify result, kept last by Identify.
    Identify,
    /// The model the engineer supplied, kept last by `importModel`.
    Supplied,
}

/// The study's current model: the model every stage reads (B3.7, A8.4), with its source and, for a fit, the Identify section it
/// came from. A supplied model has no section.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentModel {
    /// Where the model came from.
    pub source: CurrentModelSource,
    /// The model: the fitted model of the Identify section, or the supplied model.
    pub model: GeneralModel,
    /// The Identify section the fit came from; `None` for a supplied model.
    pub identify: Option<IdentifySection>,
}

impl CurrentModel {
    /// The current model of an Identify result: its fitted model, with the section the presentation and the recordings come from.
    #[must_use]
    pub fn identified(section: IdentifySection) -> Self {
        Self {
            source: CurrentModelSource::Identify,
            model: section.selected_model.clone(),
            identify: Some(section),
        }
    }

    /// The current model of a supplied model section (B3.7): the model as the engineer gave it.
    #[must_use]
    pub fn supplied(section: SuppliedModelSection) -> Self {
        Self {
            source: CurrentModelSource::Supplied,
            model: section.model,
            identify: None,
        }
    }

    /// Whether the model is provisional (B3.5, B3.7): a fit is provisional as its section says; a supplied model is provisional
    /// unless it carries evidence of independent validation. A provisional model may be tuned but cannot authorize Apply.
    #[must_use]
    pub fn provisional(&self) -> bool {
        match &self.identify {
            Some(section) => section.provisional,
            None => self.model.identity().validation != ModelValidation::Validated,
        }
    }

    /// The normalized error that decides a fit (N3.2, B4.2): the estimation error for a provisional fit and the validation error
    /// otherwise, as the study's presentation records them. `None` for a supplied model, which was not fitted, and for a fit whose
    /// presentation records no such error.
    #[must_use]
    pub fn fit_error(&self) -> Option<f64> {
        let section = self.identify.as_ref()?;
        let shown = &section.presentation.as_ref()?.presentation;
        if section.provisional {
            shown.estimation.normalized_error
        } else {
            shown.validation.normalized_error
        }
    }

    /// Whether the model is current for `loop_definition` (A8.4). A fit is current while its signals are the loop's; a supplied
    /// model is current while its study is (see the module's note on its channel names).
    #[must_use]
    pub fn is_current_for(&self, loop_definition: &LoopDefinition) -> bool {
        self.identify
            .as_ref()
            .is_none_or(|section| section.is_current_for(loop_definition))
    }
}

/// The study's current model (B3.7, A8.4): the one section it holds, or, when it holds both, the one its marker names. `None` when
/// it holds neither, as after Reset.
///
/// # Errors
/// A stored section cannot be read (B10.4).
pub fn current_model(study: &Study) -> Result<Option<CurrentModel>, Failure> {
    let identify = study.section::<IdentifySection>()?;
    let supplied = study.section::<SuppliedModelSection>()?;
    Ok(match (identify, supplied) {
        (None, None) => None,
        (Some(identify), None) => Some(CurrentModel::identified(identify)),
        (None, Some(supplied)) => Some(CurrentModel::supplied(supplied)),
        (Some(identify), Some(supplied)) => Some(match study.model_source {
            Some(CurrentModelSource::Supplied) => CurrentModel::supplied(supplied),
            Some(CurrentModelSource::Identify) | None => CurrentModel::identified(identify),
        }),
    })
}

#[cfg(test)]
#[path = "current_model_tests.rs"]
mod tests;
