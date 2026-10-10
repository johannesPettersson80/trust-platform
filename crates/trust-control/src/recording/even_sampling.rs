//! Even sampling of a recorded run (`evenlySampled` in `evenSampling.ts`): the scans of a run that a live plant's task
//! recorded at uneven intervals are placed on one even grid, so an analysis that needs continuous data can compare
//! them (A4.3). A channel named as linear is interpolated between scans; every other channel is held, as a PLC holds
//! a value between scans. The grid and the gap rule are the library's (`trust_control_math::sampling`); this module
//! checks the scan sequence, which the library does not see.

#![allow(clippy::module_name_repetitions)] // `EvenSamplingError` names the module's subject; the precedent is `model.rs`.

use trust_control_math::sampling::{evenly_sampled_grid, median_spacing, SampleError, SampleGap};

use crate::recording::sequence::steady_record_period;
use crate::signals::Quality;

/// The fewest scans an evenly sampled recording keeps (`Model validation requires at least 30 complete scans.`).
pub const MINIMUM_EVEN_SCANS: usize = 30;

/// One channel's reading at a scan: its value, or none when the scan has no value for it, and its quality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelReading {
    /// The value at the scan, or `None` when it is missing.
    pub value: Option<f64>,
    /// The quality of the value at the scan.
    pub quality: Quality,
}

/// A scan of a recorded run on the time axis: its time in seconds from the first scan, its sequence value, and one
/// reading per channel in the run's channel order.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedRecordedScan {
    /// The time of the scan in seconds.
    pub time_seconds: f64,
    /// The sequence value of the scan: the last execution the scan covers.
    pub sequence: u64,
    /// The readings of the scan, one per channel.
    pub readings: Vec<ChannelReading>,
}

/// A recorded run placed on its even grid: the scans on the grid, and the grid step in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct EvenlySampled {
    /// The scans on the grid, in time order. A run already on its grid is returned as it was recorded.
    pub scans: Vec<TimedRecordedScan>,
    /// The grid step in seconds: the requested step, or the median spacing of the scans.
    pub step_seconds: f64,
}

/// Why a recorded run cannot be placed on an even grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvenSamplingError {
    /// Fewer than [`MINIMUM_EVEN_SCANS`] scans, or fewer grid points than that.
    TooFew,
    /// The scans are out of time order, their sequence values do not step by one steady record period, or their
    /// spacing is not positive.
    Irregular,
    /// An interval is longer than one and a half steps. The first such gap is named.
    Gap(SampleGap),
    /// The grid cannot hold the scans.
    Unplaceable,
}

/// Places a recorded run on an even grid (`evenlySampled`).
///
/// The scans must hold at least [`MINIMUM_EVEN_SCANS`] values, be in strictly increasing time, step by one steady
/// record period in their sequence values (the last step may be partial), and have no interval longer than one and
/// a half steps. The step is `step_seconds`, or the median spacing when none is given. A run whose scans already lie
/// on that grid is returned unchanged. Otherwise each grid point takes the scan held at or before it, and each channel
/// named in `linear_channels` is interpolated toward the next scan when both values are present; a grid point's
/// quality becomes the next scan's quality unless the held scan's quality is not good. Grid point `k` takes the
/// sequence value of the first scan plus `k`.
///
/// # Errors
/// Returns [`EvenSamplingError::TooFew`], [`EvenSamplingError::Irregular`], [`EvenSamplingError::Gap`] or
/// [`EvenSamplingError::Unplaceable`], in that order of checks.
pub fn evenly_sampled(
    scans: &[TimedRecordedScan],
    step_seconds: Option<f64>,
    linear_channels: &[usize],
) -> Result<EvenlySampled, EvenSamplingError> {
    if scans.len() < MINIMUM_EVEN_SCANS {
        return Err(EvenSamplingError::TooFew);
    }
    let times: Vec<f64> = scans.iter().map(|scan| scan.time_seconds).collect();
    let sequences: Vec<u64> = scans.iter().map(|scan| scan.sequence).collect();
    let spacing = step_seconds.unwrap_or_else(|| median_spacing(&times));
    let increasing = times.windows(2).all(|pair| pair[1] > pair[0]);
    if spacing.is_nan() || spacing <= 0.0 || !increasing || !steady_record_period(&sequences, true)
    {
        return Err(EvenSamplingError::Irregular);
    }
    let grid = match evenly_sampled_grid(&times, Some(spacing), MINIMUM_EVEN_SCANS) {
        Ok(grid) => grid,
        Err(SampleError::TooFewSamples { .. }) => return Err(EvenSamplingError::TooFew),
        Err(SampleError::Gap(gap)) => return Err(EvenSamplingError::Gap(gap)),
        Err(SampleError::IrregularTimes) => return Err(EvenSamplingError::Irregular),
        Err(SampleError::Unplaceable | SampleError::InvalidLimit) => {
            return Err(EvenSamplingError::Unplaceable)
        }
    };
    if grid.aligned {
        return Ok(EvenlySampled {
            scans: scans.to_vec(),
            step_seconds: grid.step_seconds,
        });
    }

    let mut placed = Vec::with_capacity(grid.points.len());
    for (sequence, point) in (scans[0].sequence..).zip(&grid.points) {
        let held = &scans[point.index];
        let next = if point.fraction > 0.0 {
            scans.get(point.index + 1)
        } else {
            None
        };
        let readings = held
            .readings
            .iter()
            .enumerate()
            .map(|(channel, reading)| {
                let after = next.and_then(|next| next.readings.get(channel).copied());
                interpolated(
                    linear_channels.contains(&channel),
                    *reading,
                    after,
                    point.fraction,
                )
            })
            .collect();
        placed.push(TimedRecordedScan {
            time_seconds: point.time_seconds,
            sequence,
            readings,
        });
    }
    if placed.len() < MINIMUM_EVEN_SCANS {
        return Err(EvenSamplingError::TooFew);
    }
    Ok(EvenlySampled {
        scans: placed,
        step_seconds: grid.step_seconds,
    })
}

/// The reading of one channel at a grid point: the held reading, or for a linear channel the value interpolated toward
/// the next scan's value when both are present (`held + fraction · (next − held)`).
fn interpolated(
    linear: bool,
    held: ChannelReading,
    next: Option<ChannelReading>,
    fraction: f64,
) -> ChannelReading {
    match (linear, held.value, next) {
        (true, Some(value), Some(after)) => match after.value {
            Some(after_value) => ChannelReading {
                value: Some(value + fraction * (after_value - value)),
                quality: if held.quality == Quality::Good {
                    after.quality
                } else {
                    held.quality
                },
            },
            None => held,
        },
        _ => held,
    }
}

#[cfg(test)]
#[path = "even_sampling_tests.rs"]
mod tests;
