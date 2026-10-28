//! Even sampling of irregular scans and block averaging of long records, and the statistics over a
//! record. A recording's scans are held onto a grid at their median spacing (a PLC holds a value
//! between scans), or interpolated linearly where a caller asks. Each function states the rule it
//! applies. The caller passes the sample limits and the names of its channels; the sentences of its
//! refusals belong to the product.

use crate::identify::SampleQuality;

pub mod plot_extrema;
pub mod statistics;

use self::statistics::mean;

/// One timed sample of a fit: its time, its input and output, its quality, and whether it was
/// clipped or saturated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvenSample {
    /// The time of the sample in seconds.
    pub time_seconds: f64,
    /// The input at the sample.
    pub input: f64,
    /// The output at the sample.
    pub output: f64,
    /// The quality of the sample.
    pub quality: SampleQuality,
    /// Whether the input was clipped at the sample.
    pub clipped: bool,
    /// Whether the input was saturated at the sample.
    pub saturated: bool,
}

/// An interval longer than one and a half steps, which an analysis that needs continuous data
/// cannot bridge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleGap {
    /// The time at which the gap starts, the sample before it.
    pub start_seconds: f64,
    /// The length of the gap in seconds.
    pub length_seconds: f64,
}

/// A point of an even grid: its time, the index of the last sample at or before it, and its
/// fraction of the way from that sample to the next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridPoint {
    /// The grid time in seconds.
    pub time_seconds: f64,
    /// The index of the last sample at or before the grid time.
    pub index: usize,
    /// The fraction in `[0, 1]` of the way from that sample to the next; zero at the last sample.
    pub fraction: f64,
}

/// A recording placed on an even grid.
#[derive(Debug, Clone, PartialEq)]
pub struct EvenGrid {
    /// The grid step in seconds: the requested step, or the median spacing.
    pub step_seconds: f64,
    /// Whether the scans already lie on the grid, within `10⁻⁸` s, so the points are the scans.
    pub aligned: bool,
    /// The grid points, in time order.
    pub points: Vec<GridPoint>,
}

/// Why a recording cannot be sampled or averaged.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SampleError {
    /// Fewer samples than the caller requires.
    #[error("too few samples")]
    TooFewSamples {
        /// The number of samples the caller requires.
        minimum: usize,
    },
    /// The times are not strictly increasing, or the step is not positive.
    #[error("times are not strictly increasing or the step is not positive")]
    IrregularTimes,
    /// An interval is longer than one and a half steps.
    #[error("gap longer than one and a half steps")]
    Gap(SampleGap),
    /// The grid cannot hold the times: its step is not positive, its points are not finite, or it
    /// has more than `4n + 16` points.
    #[error("cannot place the record on an even grid")]
    Unplaceable,
    /// The block limit is zero.
    #[error("block limit must be at least one")]
    InvalidLimit,
}

/// The median of the consecutive intervals `t[i + 1] − t[i]`: the element at index `⌊n/2⌋` of the
/// sorted intervals, so the upper median for an even count. Zero when there are fewer than two
/// times.
#[must_use]
pub fn median_spacing(times: &[f64]) -> f64 {
    let mut intervals: Vec<f64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
    intervals.sort_by(f64::total_cmp);
    intervals.get(intervals.len() / 2).copied().unwrap_or(0.0)
}

/// The first interval `t[i] − t[i − 1]` longer than `1.5·step + 10⁻⁹` seconds, which marks a gap
/// that continuous data cannot bridge; `None` when no interval is that long.
#[must_use]
pub fn first_gap(times: &[f64], step_seconds: f64) -> Option<SampleGap> {
    times.windows(2).find_map(|pair| {
        let length_seconds = pair[1] - pair[0];
        (length_seconds > 1.5 * step_seconds + 1e-9).then_some(SampleGap {
            start_seconds: pair[0],
            length_seconds,
        })
    })
}

/// The grid times `t₀ + k·step` for `k = 0, …, ⌊(t_last − t₀)/step + 10⁻⁹⌋`. Each point holds the
/// index `i` of the last sample at or before its time (the sample `t[i] ≤ g + 10⁻⁹`) and its
/// fraction `(g − t[i])/(t[i + 1] − t[i])`, clamped to `[0, 1]`. The last sample gives fraction zero.
///
/// # Errors
/// Returns [`SampleError::Unplaceable`] when the step is not positive, the grid is not finite,
/// or it has more than `4n + 16` points for `n` samples.
pub fn even_grid(times: &[f64], step_seconds: f64) -> Result<Vec<GridPoint>, SampleError> {
    let (Some(&first), Some(&last)) = (times.first(), times.last()) else {
        return Err(SampleError::Unplaceable);
    };
    if step_seconds.is_nan() || step_seconds <= 0.0 {
        return Err(SampleError::Unplaceable);
    }
    let count = ((last - first) / step_seconds + 1e-9).floor() + 1.0;
    if !count.is_finite() || count > 4.0 * as_f64(times.len()) + 16.0 {
        return Err(SampleError::Unplaceable);
    }
    let points = count_to_points(count);
    let mut grid = Vec::with_capacity(points);
    let mut index = 0;
    for step in 0..points {
        let time_seconds = first + as_f64(step) * step_seconds;
        while index + 1 < times.len() && times[index + 1] <= time_seconds + 1e-9 {
            index += 1;
        }
        let fraction = if index + 1 < times.len() {
            ((time_seconds - times[index]) / (times[index + 1] - times[index])).clamp(0.0, 1.0)
        } else {
            0.0
        };
        grid.push(GridPoint {
            time_seconds,
            index,
            fraction,
        });
    }
    Ok(grid)
}

/// Checks the times and places them on an even grid at `step_seconds`, or at their median spacing
/// when no step is given. The times must be strictly increasing, the step positive and no interval
/// longer than `1.5·step`. When every scan lies on the grid within `10⁻⁸` s (`t_i = t₀ + i·step`),
/// the grid is the scans themselves (`aligned`), and otherwise [`even_grid`] places it.
///
/// # Errors
/// Returns [`SampleError::TooFewSamples`] for fewer than `minimum_samples` times or grid points,
/// [`SampleError::IrregularTimes`], [`SampleError::Gap`] with the first gap, and
/// [`SampleError::Unplaceable`].
pub fn evenly_sampled_grid(
    times: &[f64],
    step_seconds: Option<f64>,
    minimum_samples: usize,
) -> Result<EvenGrid, SampleError> {
    let too_few = SampleError::TooFewSamples {
        minimum: minimum_samples,
    };
    if times.len() < minimum_samples {
        return Err(too_few);
    }
    let Some(&first) = times.first() else {
        return Err(too_few);
    };
    let step = step_seconds.unwrap_or_else(|| median_spacing(times));
    if step.is_nan() || step <= 0.0 || times.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(SampleError::IrregularTimes);
    }
    if let Some(gap) = first_gap(times, step) {
        return Err(SampleError::Gap(gap));
    }
    let aligned = times
        .iter()
        .enumerate()
        .all(|(index, time)| (time - first - as_f64(index) * step).abs() <= 1e-8);
    let points = if aligned {
        times
            .iter()
            .enumerate()
            .map(|(index, &time_seconds)| GridPoint {
                time_seconds,
                index,
                fraction: 0.0,
            })
            .collect()
    } else {
        even_grid(times, step)?
    };
    if points.len() < minimum_samples {
        return Err(too_few);
    }
    Ok(EvenGrid {
        step_seconds: step,
        aligned,
        points,
    })
}

/// The linear interpolation `held + fraction·(next − held)`: the value the fraction of the way from
/// `held` to `next`.
#[must_use]
pub fn interpolate(held: f64, next: f64, fraction: f64) -> f64 {
    held + fraction * (next - held)
}

/// Resamples a recording onto its even grid at `step_seconds` (default: the median spacing). An
/// input is held from the last sample at or before each grid time. An output is interpolated
/// linearly toward the next sample. A grid point is bad, clipped or saturated when any sample it
/// uses is. A recording already on its grid (every interval within `10⁻⁶` of the step) is returned
/// unchanged.
///
/// # Errors
/// Returns [`SampleError::Unplaceable`] when the grid cannot be placed.
pub fn resampled_evenly(
    samples: &[EvenSample],
    step_seconds: Option<f64>,
) -> Result<Vec<EvenSample>, SampleError> {
    let times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let step = step_seconds.unwrap_or_else(|| median_spacing(&times));
    let on_grid = times
        .windows(2)
        .all(|pair| ((pair[1] - pair[0]) - step).abs() <= 1e-6 * step);
    if on_grid {
        return Ok(samples.to_vec());
    }
    let points = even_grid(&times, step)?;
    Ok(points
        .iter()
        .map(|point| {
            let held = samples[point.index];
            let next = if point.fraction > 0.0 {
                samples.get(point.index + 1).copied()
            } else {
                None
            };
            let output = match next {
                Some(next) => interpolate(held.output, next.output, point.fraction),
                None => held.output,
            };
            let members: Vec<EvenSample> = next.map_or_else(|| vec![held], |next| vec![held, next]);
            let (quality, clipped, saturated) = combined_flags(&members);
            EvenSample {
                time_seconds: point.time_seconds,
                input: held.input,
                output,
                quality,
                clipped,
                saturated,
            }
        })
        .collect())
}

/// Block-averages a recording to at most `maximum_points` samples before a fit. A block takes the
/// mean time, input and output of its members, and it is bad, clipped or saturated when any member
/// is. A cut time (a step) splits blocks: a sample's side is the number of cuts at or before its
/// time, and no block holds samples from two sides. The block size is the smallest that keeps the
/// count within the limit: `⌈n/maximum⌉`, raised by one while the cuts leave more blocks than the
/// limit allows.
///
/// # Errors
/// Returns [`SampleError::InvalidLimit`] when `maximum_points` is zero.
pub fn block_averaged(
    samples: &[EvenSample],
    maximum_points: usize,
    cuts_seconds: &[f64],
) -> Result<Vec<EvenSample>, SampleError> {
    if maximum_points == 0 {
        return Err(SampleError::InvalidLimit);
    }
    if samples.len() <= maximum_points {
        return Ok(samples.to_vec());
    }
    let mut size = samples.len().div_ceil(maximum_points);
    let mut starts = block_starts(samples, size, cuts_seconds);
    while starts.len() > maximum_points && size < samples.len() {
        size += 1;
        starts = block_starts(samples, size, cuts_seconds);
    }
    Ok(starts
        .iter()
        .enumerate()
        .map(|(block, &start)| {
            let end = starts.get(block + 1).copied().unwrap_or(samples.len());
            let members = &samples[start..end];
            let times: Vec<f64> = members.iter().map(|sample| sample.time_seconds).collect();
            let inputs: Vec<f64> = members.iter().map(|sample| sample.input).collect();
            let outputs: Vec<f64> = members.iter().map(|sample| sample.output).collect();
            let (quality, clipped, saturated) = combined_flags(members);
            EvenSample {
                time_seconds: mean(&times),
                input: mean(&inputs),
                output: mean(&outputs),
                quality,
                clipped,
                saturated,
            }
        })
        .collect())
}

/// The largest value, or `floor` when no value exceeds it.
#[must_use]
pub fn largest(values: &[f64], floor: f64) -> f64 {
    values.iter().fold(
        floor,
        |result, &value| if value > result { value } else { result },
    )
}

/// The smallest value; positive infinity when there are no values.
#[must_use]
pub fn smallest(values: &[f64]) -> f64 {
    values.iter().fold(
        f64::INFINITY,
        |result, &value| if value < result { value } else { result },
    )
}

/// The root sum of squares `√(Σ vᵢ²)`, the Euclidean norm of the values.
#[must_use]
pub fn root_sum_square(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

/// The first index of each block: a block starts at a sample and extends while its size allows and
/// its samples stay on its side of the cuts.
fn block_starts(samples: &[EvenSample], size: usize, cuts_seconds: &[f64]) -> Vec<usize> {
    let cut_side = |time: f64| cuts_seconds.iter().filter(|&&cut| cut <= time).count();
    let mut indices = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        indices.push(start);
        let block_side = cut_side(samples[start].time_seconds);
        let mut end = start + 1;
        while end < samples.len()
            && end - start < size
            && cut_side(samples[end].time_seconds) == block_side
        {
            end += 1;
        }
        start = end;
    }
    indices
}

/// The quality, clipped and saturated flags of a group of samples. The group is bad when any member
/// is bad, clipped or saturated; otherwise uncertain when any member is uncertain; otherwise good.
fn combined_flags(members: &[EvenSample]) -> (SampleQuality, bool, bool) {
    let flagged = members
        .iter()
        .any(|sample| sample.quality == SampleQuality::Bad || sample.clipped || sample.saturated);
    let quality = if flagged {
        SampleQuality::Bad
    } else if members
        .iter()
        .any(|sample| sample.quality == SampleQuality::Uncertain)
    {
        SampleQuality::Uncertain
    } else {
        SampleQuality::Good
    };
    (
        quality,
        members.iter().any(|sample| sample.clipped),
        members.iter().any(|sample| sample.saturated),
    )
}

/// A count as a float. The counts here are sample counts, far below 2^53.
#[allow(clippy::cast_precision_loss)]
fn as_f64(count: usize) -> f64 {
    count as f64
}

/// The number of grid points for a grid count: the count is finite and at least one here (or the
/// grid is empty when the times run backwards).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn count_to_points(count: f64) -> usize {
    count.max(0.0) as usize
}

#[cfg(test)]
mod tests;
