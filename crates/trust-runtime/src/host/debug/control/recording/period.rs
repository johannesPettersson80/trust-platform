//! The record period of a recording (specification 35, A4.2): the executions one sample averages.
use super::super::signals::{Captured, Quality};
use super::IoRecordingSample;
use crate::control::search::bounded_mean;

/// The executions folded into the sample being built, one channel at a time.
#[derive(Debug, Default)]
pub(super) struct RecordPeriod {
    executions: u32,
    gap: bool,
    /// The sum of the executions' times in nanoseconds; `None` once one execution had no time.
    time_sum_ns: Option<i128>,
    captured_at_ms: u64,
    channels: Vec<ChannelFold>,
}

#[derive(Debug, Clone)]
struct ChannelFold {
    /// The sum of the executions' values; `None` once one execution had no value.
    sum: Option<f64>,
    /// The smallest and the largest of the executions' values, which bound their mean.
    range: Option<(f64, f64)>,
    /// The latest execution's value: the sample of a BOOL channel (A4.2).
    last: Option<f64>,
    forced: bool,
    /// The worst quality of the executions (A4.2).
    quality: Quality,
}

impl RecordPeriod {
    pub(super) fn executions(&self) -> u32 {
        self.executions
    }

    /// Notes a missed execution that falls inside the period. One that comes before the period's
    /// first execution lies between two samples and marks neither of them (A4.4).
    pub(super) fn mark_gap(&mut self) {
        if self.executions > 0 {
            self.gap = true;
        }
    }

    pub(super) fn add(&mut self, time_ns: Option<i64>, captured: Captured, captured_at_ms: u64) {
        let readings = captured
            .values
            .into_iter()
            .zip(captured.forced)
            .zip(captured.quality);
        if self.executions == 0 {
            self.time_sum_ns = time_ns.map(i128::from);
            self.channels = readings
                .map(|((value, forced), quality)| ChannelFold {
                    sum: value,
                    range: value.map(|value| (value, value)),
                    last: value,
                    forced,
                    quality,
                })
                .collect();
        } else {
            self.time_sum_ns = self
                .time_sum_ns
                .zip(time_ns)
                .map(|(sum, time)| sum + i128::from(time));
            for (channel, ((value, forced), quality)) in self.channels.iter_mut().zip(readings) {
                channel.sum = channel.sum.zip(value).map(|(sum, value)| sum + value);
                channel.range = channel
                    .range
                    .zip(value)
                    .map(|((smallest, largest), value)| (smallest.min(value), largest.max(value)));
                channel.last = value;
                channel.forced |= forced;
                channel.quality = worse(channel.quality.clone(), quality);
            }
        }
        self.executions += 1;
        self.captured_at_ms = captured_at_ms;
    }

    /// The sample of the period: each channel's mean (its last value for BOOL), its worst quality
    /// and whether any execution was forced, and the mean of the execution times.
    pub(super) fn finish(self, scan: u64, boolean: &[bool]) -> IoRecordingSample {
        let count = f64::from(self.executions);
        let mut values = Vec::with_capacity(self.channels.len());
        let mut forced = Vec::with_capacity(self.channels.len());
        let mut quality = Vec::with_capacity(self.channels.len());
        for (channel, boolean) in self.channels.into_iter().zip(boolean) {
            let mean = if *boolean {
                channel.last
            } else {
                channel
                    .sum
                    .zip(channel.range)
                    .map(|(sum, (smallest, largest))| bounded_mean(sum / count, smallest, largest))
            };
            let (value, worst) = match mean {
                Some(value) if value.is_finite() => (Some(value), channel.quality),
                _ => (None, worse(channel.quality, Quality::Bad)),
            };
            values.push(value);
            forced.push(channel.forced);
            quality.push(worst);
        }
        IoRecordingSample {
            scan,
            runtime_time_ns: self
                .time_sum_ns
                .and_then(|sum| i64::try_from(sum / i128::from(self.executions)).ok()),
            captured_at_ms: self.captured_at_ms,
            values,
            forced,
            quality,
            gap: self.gap,
        }
    }
}

/// The worse of two qualities. The declaration order of `Quality` is the severity order (good,
/// bad, forced, missing), so a later variant is worse.
fn worse(first: Quality, second: Quality) -> Quality {
    if rank(&second) > rank(&first) {
        second
    } else {
        first
    }
}

fn rank(quality: &Quality) -> u8 {
    match quality {
        Quality::Good => 0,
        Quality::Bad => 1,
        Quality::Forced => 2,
        Quality::Missing => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(value: Option<f64>, quality: Quality, forced: bool) -> Captured {
        Captured {
            values: vec![value],
            forced: vec![forced],
            quality: vec![quality],
        }
    }

    #[test]
    fn quality_is_the_worst_in_declaration_order() {
        assert!(matches!(
            worse(Quality::Good, Quality::Forced),
            Quality::Forced
        ));
        assert!(matches!(
            worse(Quality::Forced, Quality::Bad),
            Quality::Forced
        ));
        assert!(matches!(
            worse(Quality::Missing, Quality::Forced),
            Quality::Missing
        ));
    }

    #[test]
    fn a_held_value_comes_back_exactly_from_a_record_period() {
        // 0.1 + 0.1 + 0.1 is 0.30000000000000004 in floating point, so its sum divided by three is not 0.1 (A4.2).
        let mut period = RecordPeriod::default();
        for scan in 0..3 {
            period.add(Some(scan), captured(Some(0.1), Quality::Good, false), 0);
        }
        assert_eq!(period.finish(3, &[false]).values, vec![Some(0.1)]);
    }

    #[test]
    fn a_missed_execution_marks_only_a_period_that_already_holds_one() {
        let mut inside = RecordPeriod::default();
        inside.add(Some(1), captured(Some(1.0), Quality::Good, false), 0);
        inside.mark_gap();
        assert!(inside.finish(1, &[false]).gap);

        let mut between = RecordPeriod::default();
        between.mark_gap();
        between.add(Some(2), captured(Some(2.0), Quality::Good, false), 0);
        assert!(!between.finish(2, &[false]).gap);
    }
}
