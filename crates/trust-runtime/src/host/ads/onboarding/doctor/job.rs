use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::*;

/// Shared cancellation token for doctor jobs.
#[derive(Debug, Clone, Default)]
pub struct DoctorCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DoctorCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Progress snapshot for a doctor job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorJobProgress {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub current_step: Option<DoctorStepId>,
}

impl DoctorJobProgress {
    pub fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            current_step: None,
        }
    }
}

/// Lifecycle state for a doctor job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorJobState {
    Queued,
    Running,
    Complete,
    Cancelled,
}

/// Minimal engine-owned doctor job runner.
#[derive(Debug, Clone)]
pub struct DoctorJob {
    state: DoctorJobState,
    progress: DoctorJobProgress,
    cancellation: DoctorCancellation,
    report: Option<DoctorReport>,
}

impl DoctorJob {
    pub fn new() -> Self {
        Self {
            state: DoctorJobState::Queued,
            progress: DoctorJobProgress::new(REQUIRED_DOCTOR_STEPS.len()),
            cancellation: DoctorCancellation::new(),
            report: None,
        }
    }

    pub fn state(&self) -> DoctorJobState {
        self.state
    }

    pub fn progress(&self) -> &DoctorJobProgress {
        &self.progress
    }

    pub fn cancellation(&self) -> DoctorCancellation {
        self.cancellation.clone()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn report(&self) -> Option<&DoctorReport> {
        self.report.as_ref()
    }

    pub fn run<W: AdsOnboardingWire>(
        &mut self,
        wire: &mut W,
        options: DoctorOptions,
    ) -> DoctorReport {
        self.state = DoctorJobState::Running;
        self.progress = DoctorJobProgress::new(REQUIRED_DOCTOR_STEPS.len());
        let report =
            run_doctor_with_progress(wire, options, &self.cancellation, &mut self.progress);
        self.state = if self.cancellation.is_cancelled() {
            DoctorJobState::Cancelled
        } else {
            DoctorJobState::Complete
        };
        self.report = Some(report.clone());
        report
    }
}

impl Default for DoctorJob {
    fn default() -> Self {
        Self::new()
    }
}
