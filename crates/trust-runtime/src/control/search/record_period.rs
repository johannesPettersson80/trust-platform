//! Record periods of an isolated run (specification 35, A4.2): one sample is the mean of the
//! executions it averages, and its time is the mean of their grid times.

/// The readings folded into the sample being built.
#[derive(Default)]
pub(super) struct RecordPeriod {
    executions: u64,
    sums: Vec<f64>,
    smallest: Vec<f64>,
    largest: Vec<f64>,
}

impl RecordPeriod {
    /// Folds one grid point's readings, one per observed channel in channel order.
    pub(super) fn push(&mut self, readings: &[f64]) {
        if self.executions == 0 {
            self.sums = readings.to_vec();
            self.smallest = readings.to_vec();
            self.largest = readings.to_vec();
        } else {
            for (channel, reading) in readings.iter().enumerate() {
                self.sums[channel] += reading;
                self.smallest[channel] = self.smallest[channel].min(*reading);
                self.largest[channel] = self.largest[channel].max(*reading);
            }
        }
        self.executions += 1;
    }

    /// The mean of each channel's readings, in channel order.
    pub(super) fn means(&self) -> Vec<f64> {
        (0..self.sums.len())
            .map(|channel| {
                bounded_mean(
                    self.sums[channel] / self.executions as f64,
                    self.smallest[channel],
                    self.largest[channel],
                )
            })
            .collect()
    }
}

/// A mean kept between the smallest and the largest reading it averages. Rounding in the sum can
/// carry it just outside them, and a held value must come back exactly (A4.2). A non-finite mean
/// is returned as it is.
pub(crate) fn bounded_mean(mean: f64, smallest: f64, largest: f64) -> f64 {
    if mean < smallest {
        smallest
    } else if mean > largest {
        largest
    } else {
        mean
    }
}

/// How many record periods `points` grid points form.
pub(super) fn period_count(points: u64, record_every: u64) -> u64 {
    points.div_ceil(record_every)
}

/// The time of record period `period` in seconds: the mean of its grid times. The grid is evenly
/// spaced, so that mean is `(first + last) * step / 2` milliseconds, exact in floating point; a
/// period of one grid point is exactly that grid time.
pub(super) fn period_time_seconds(
    period: u64,
    record_every: u64,
    last_point: u64,
    step_ms: u64,
) -> f64 {
    let first = period * record_every;
    let last = (first + record_every - 1).min(last_point);
    ((first + last) * step_ms) as f64 / 2.0 / 1000.0
}
