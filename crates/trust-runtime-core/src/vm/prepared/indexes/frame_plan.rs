//! Prepared phase ranges for per-invocation frame initialization.
use super::*;
use crate::bytecode::{InitializationPhase, InitializationTrigger};
use core::ops::Range;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct FrameActionPlan {
    parameter: ActionRange,
    return_value: ActionRange,
    static_ordinary: ActionRange,
    static_after_restart: ActionRange,
    frame: ActionRange,
}

impl FrameActionPlan {
    pub(super) fn range(
        &self,
        phase: InitializationPhase,
        trigger: InitializationTrigger,
    ) -> Range<usize> {
        match (phase, trigger) {
            (InitializationPhase::Parameter, InitializationTrigger::Ordinary) => {
                self.parameter.as_range()
            }
            (InitializationPhase::Return, InitializationTrigger::Ordinary) => {
                self.return_value.as_range()
            }
            (InitializationPhase::Static, InitializationTrigger::Ordinary) => {
                self.static_ordinary.as_range()
            }
            (InitializationPhase::Static, InitializationTrigger::AfterRestart) => {
                self.static_after_restart.as_range()
            }
            (InitializationPhase::Frame, InitializationTrigger::Ordinary) => self.frame.as_range(),
            _ => ActionRange::EMPTY.as_range(),
        }
    }

    fn set_group(
        &mut self,
        phase: u8,
        trigger: u8,
        range: Range<usize>,
    ) -> Result<(), RuntimeError> {
        let target = match (phase, trigger) {
            (phase, trigger)
                if phase == InitializationPhase::Parameter as u8
                    && trigger == InitializationTrigger::Ordinary as u8 =>
            {
                &mut self.parameter
            }
            (phase, trigger)
                if phase == InitializationPhase::Return as u8
                    && trigger == InitializationTrigger::Ordinary as u8 =>
            {
                &mut self.return_value
            }
            (phase, trigger)
                if phase == InitializationPhase::Static as u8
                    && trigger == InitializationTrigger::Ordinary as u8 =>
            {
                &mut self.static_ordinary
            }
            (phase, trigger)
                if phase == InitializationPhase::Static as u8
                    && trigger == InitializationTrigger::AfterRestart as u8 =>
            {
                &mut self.static_after_restart
            }
            (phase, trigger)
                if phase == InitializationPhase::Frame as u8
                    && trigger == InitializationTrigger::Ordinary as u8 =>
            {
                &mut self.frame
            }
            _ => return Ok(()),
        };
        *target = ActionRange::from_range(range)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ActionRange {
    start: u32,
    end: u32,
}

impl ActionRange {
    const EMPTY: Self = Self { start: 0, end: 0 };

    fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    fn from_range(range: Range<usize>) -> Result<Self, RuntimeError> {
        Ok(Self {
            start: u32::try_from(range.start).map_err(|_| RuntimeError::Overflow)?,
            end: u32::try_from(range.end).map_err(|_| RuntimeError::Overflow)?,
        })
    }
}

impl PreparedIndexes {
    pub(super) fn prepare_frame_action_plans(
        &mut self,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        let mut owner_count = 0usize;
        let mut last_owner = None;
        for (key, _) in self.actions.groups() {
            budget.charge(0, 1)?;
            if let Some(owner) = frame_action_owner(key)
                && last_owner != Some(owner)
            {
                owner_count = owner_count.checked_add(1).ok_or(RuntimeError::Overflow)?;
                last_owner = Some(owner);
            }
        }

        budget.records::<(u32, FrameActionPlan)>(owner_count)?;
        self.frame_action_plans
            .try_reserve_exact(owner_count)
            .map_err(|_| RuntimeError::Overflow)?;
        last_owner = None;
        for (key, range) in self.actions.groups() {
            budget.charge(0, 1)?;
            let Some(owner) = frame_action_owner(key) else {
                continue;
            };
            if last_owner != Some(owner) {
                self.frame_action_plans
                    .push((owner, FrameActionPlan::default()));
                last_owner = Some(owner);
            }
            let (_, phase, trigger) = key.parts();
            let Some((_, plan)) = self.frame_action_plans.last_mut() else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            plan.set_group(phase, trigger, range)?;
        }
        Ok(())
    }
}

fn frame_action_owner(key: ActionKey) -> Option<u32> {
    let (owner, phase, _) = key.parts();
    let owner = owner?;
    matches!(
        phase,
        value if value == InitializationPhase::Parameter as u8
            || value == InitializationPhase::Return as u8
            || value == InitializationPhase::Static as u8
            || value == InitializationPhase::Frame as u8
    )
    .then_some(owner)
}
