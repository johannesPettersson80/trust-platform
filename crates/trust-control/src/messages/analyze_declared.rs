//! The answers of the analysis tools that take a declared analysis or a feedback loop (`rootLocus`, `routh`, `feedback` and
//! `measuredFrequency`, B9.9). Each answer is the calculated result itself, with its two sentence lists `summary` (the result
//! area, warnings first) and `details` (the details disclosure), so a result that a study keeps (B10.2) is the answer it gave.

use crate::analysis::declared::DeclaredRootLocusResult;
use crate::analysis::general_feedback::FeedbackResult;
use crate::analysis::measured_frequency::MeasuredFrequencyResult;
use crate::analysis::routh::DeclaredRouthResult;

/// The answer of `rootLocus`: the declared pole paths at each gain of the sweep and across the declared range (N7.5, N7.6).
pub type RootLocusAnswer = DeclaredRootLocusResult;

/// The answer of `routh`: the Routh–Hurwitz reading of the characteristic polynomial and the stable interval (N7.1 to N7.4).
pub type RouthAnswer = DeclaredRouthResult;

/// The answer of `feedback`: the open loop, the sensitivities, the Nyquist count, the closed-loop poles and the margins (N4.4,
/// N7.7 to N7.11).
pub type FeedbackAnswer = FeedbackResult;

/// The answer of `measuredFrequency`: the tones of a retained periodic recording (N2.8).
pub type MeasuredFrequencyAnswer = MeasuredFrequencyResult;
