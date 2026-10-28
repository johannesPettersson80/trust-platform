//! The runtime's own trend history (specification 39, §5.9 item 1): each value a trend of the
//! active bundle shows, kept for 24 hours from the program's calls of `Hmi.Display`, on by default
//! and in memory: a sample a second for the last hour, then each minute's lowest and highest
//! value. Longer history goes through the OpenOT database.

use std::collections::{BTreeMap, VecDeque};

/// An hour in milliseconds: the samples a second are kept this long.
pub const HOUR_MS: u64 = 3_600_000;
/// A day in milliseconds: the minutes are kept this long.
pub const DAY_MS: u64 = 86_400_000;
/// A minute in milliseconds.
const MINUTE_MS: u64 = 60_000;

/// Milliseconds since the Unix epoch, the time of a sample.
pub(crate) fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// The kept values, by path as the pages write it.
#[derive(Debug, Default)]
pub struct TrendHistory {
    tracks: BTreeMap<String, Track>,
}

#[derive(Debug, Default)]
struct Track {
    /// A sample a second, for the last hour.
    recent: VecDeque<(u64, f64)>,
    /// Each older minute's start and its lowest and highest value, for the last day.
    minutes: VecDeque<Minute>,
}

#[derive(Debug, Clone, Copy)]
struct Minute {
    start: u64,
    low: f64,
    high: f64,
}

impl TrendHistory {
    /// The values to keep; a value no longer trended is forgotten.
    pub fn keep(&mut self, paths: &[String]) {
        self.tracks.retain(|path, _| paths.contains(path));
        for path in paths {
            self.tracks.entry(path.clone()).or_default();
        }
    }

    /// Whether `path` is kept.
    pub fn keeps(&self, path: &str) -> bool {
        self.tracks.contains_key(path)
    }

    /// A value read at `now_ms`; the latest of each second is kept.
    pub fn record(&mut self, path: &str, now_ms: u64, value: f64) {
        let Some(track) = self.tracks.get_mut(path) else {
            return;
        };
        if !value.is_finite() {
            return;
        }
        match track.recent.back_mut() {
            // A clock set back: the older samples stay as they are.
            Some(last) if last.0 > now_ms => return,
            Some(last) if last.0 / 1_000 == now_ms / 1_000 => *last = (now_ms, value),
            _ => track.recent.push_back((now_ms, value)),
        }
        while let Some(&(time, value)) = track.recent.front() {
            if now_ms.saturating_sub(time) <= HOUR_MS {
                break;
            }
            track.recent.pop_front();
            let start = time - time % MINUTE_MS;
            match track.minutes.back_mut() {
                Some(minute) if minute.start == start => {
                    minute.low = minute.low.min(value);
                    minute.high = minute.high.max(value);
                }
                _ => track.minutes.push_back(Minute {
                    start,
                    low: value,
                    high: value,
                }),
            }
        }
        while track
            .minutes
            .front()
            .is_some_and(|minute| now_ms.saturating_sub(minute.start) > DAY_MS)
        {
            track.minutes.pop_front();
        }
    }

    /// The samples from `since_ms` on, oldest first: each older minute as its lowest and its
    /// highest value, then a sample a second; none for a value that is not kept.
    pub fn samples(&self, path: &str, since_ms: u64) -> Vec<(u64, f64)> {
        let Some(track) = self.tracks.get(path) else {
            return Vec::new();
        };
        let mut samples = Vec::new();
        for minute in track
            .minutes
            .iter()
            .filter(|minute| minute.start + MINUTE_MS > since_ms)
        {
            samples.push((minute.start, minute.low));
            if minute.high != minute.low {
                samples.push((minute.start + MINUTE_MS / 2, minute.high));
            }
        }
        samples.extend(
            track
                .recent
                .iter()
                .copied()
                .filter(|(time, _)| *time >= since_ms),
        );
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(paths: &[&str]) -> TrendHistory {
        let mut history = TrendHistory::default();
        history.keep(
            &paths
                .iter()
                .map(|path| path.to_string())
                .collect::<Vec<_>>(),
        );
        history
    }

    #[test]
    fn a_value_is_kept_once_a_second_and_only_while_it_is_trended() {
        let mut history = kept(&["Main.T"]);
        history.record("Main.T", 1_000, 1.0);
        history.record("Main.T", 1_250, 2.0);
        history.record("Main.T", 2_100, 3.0);
        history.record("Main.Other", 2_100, 9.0);
        assert_eq!(
            history.samples("Main.T", 0),
            vec![(1_250, 2.0), (2_100, 3.0)]
        );
        assert_eq!(history.samples("Main.T", 2_000), vec![(2_100, 3.0)]);
        assert!(history.samples("Main.Other", 0).is_empty());
        assert!(history.keeps("Main.T") && !history.keeps("Main.Other"));
        history.keep(&[]);
        assert!(
            history.samples("Main.T", 0).is_empty(),
            "forgotten once no trend shows it"
        );
    }

    #[test]
    fn after_an_hour_samples_fold_into_each_minutes_lowest_and_highest_and_after_a_day_go() {
        let mut history = kept(&["Main.T"]);
        for second in 0..60u32 {
            history.record("Main.T", u64::from(second) * 1_000, f64::from(second));
        }
        history.record("Main.T", HOUR_MS + 61_000, 100.0);
        let samples = history.samples("Main.T", 0);
        assert_eq!(&samples[..2], &[(0, 0.0), (30_000, 59.0)], "{samples:?}");
        assert_eq!(samples.last(), Some(&(HOUR_MS + 61_000, 100.0)));
        assert_eq!(samples.len(), 3, "{samples:?}");
        history.record("Main.T", DAY_MS + 120_000, 7.0);
        let later = history.samples("Main.T", 0);
        assert!(
            later.iter().all(|(time, _)| *time >= 60_000),
            "a day later minute 0 is gone: {later:?}"
        );
    }
}
