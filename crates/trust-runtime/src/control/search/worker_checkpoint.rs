//! Bounded private sample frames between an isolated worker and its supervisor.
//!
//! A run's samples use the compact form of specification 35, A7: each channel name appears once,
//! in the header's `channels`, and each sample appears once, with its values as arrays in channel
//! order:
//!
//! ```text
//! {"quality":[0,2],"sequence":7,"timeSeconds":0.007,"values":[1.5,null]}
//! ```
//!
//! `quality` holds one code per channel, an index into `QUALITY_NAMES`. It is omitted when every
//! channel is `good`. A `null` value has no measurement, so its quality is never `good`. The client
//! expands this form into per-channel `{value, quality}` objects (`recordedRunWire.ts`).
//!
//! The supervisor assembles a streamed run from its frames: the final result repeats the header
//! with no samples, and `read_output` attaches the received samples to it.
use super::execution_settings::{MAX_CHANNEL_SAMPLES, MAX_RUN_SAMPLES};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

const HEADER: &str = "TRUST_RUN_HEADER ";
const SAMPLE: &str = "TRUST_RUN_SAMPLE ";

/// Quality codes of a wire sample: an index into this table, the client's quality vocabulary.
pub(super) const QUALITY_NAMES: [&str; 4] = ["good", "bad", "forced", "missing"];
pub(super) const QUALITY_GOOD: u64 = 0;
pub(super) const QUALITY_BAD: u64 = 1;

// Byte budgets (specification 35, A7). Each size bounds what the encoder writes.
/// The longest JSON number an f64 writes, such as `-1.2345678901234567e-300` (24 bytes).
const NUMBER_BYTES: u64 = 24;
/// One separator after an array element.
const SEPARATOR_BYTES: u64 = 1;
/// One quality code: a single digit (codes 0 to 3).
const CODE_BYTES: u64 = 1;
/// One channel of a sample: its value and its quality code, each followed by a separator.
pub(super) const VALUE_BYTES: u64 = NUMBER_BYTES + SEPARATOR_BYTES + CODE_BYTES + SEPARATOR_BYTES;
/// One sample object without its channel entries: the keys (`{"quality":[],"sequence":`, then
/// `,"timeSeconds":`, `,"values":[]}`), a sequence of at most five digits (below 100,000), a time of
/// at most `NUMBER_BYTES`, and the separator before the next sample. 83 bytes.
pub(super) const SAMPLE_BYTES: u64 = 83;
/// One planned time of the header's `sampleTimesSeconds`, with its separator.
pub(super) const TIME_BYTES: u64 = NUMBER_BYTES + SEPARATOR_BYTES;
/// A frame's prefix (`TRUST_RUN_SAMPLE `, 17 bytes, or the header's prefix of the same length) and
/// its newline.
pub(super) const FRAME_BYTES: u64 = 18;
/// Header metadata other than the planned times, rounded up to 512 KiB. The admitted maxima are at
/// most 1024 events of 256-byte symbols (341 bytes each), 64 initial values and 64 parameters (284
/// bytes each), 64 channels with 256-byte names and units (532 bytes each), and provenance, revisions
/// and the feedback relay (under 10 KiB together). The sum is about 416 KiB.
pub(super) const HEADER_BYTES: u64 = 512 * 1024;
/// Slack for a final result's status, reasons and diagnostics.
pub(super) const RESERVE_BYTES: u64 = 1024 * 1024;
/// Checkpoint bytes one run may write (A7): its header, then one frame per sample. The bound is
/// `S × (83 + 18 + 25) + L × 27 + 512 KiB`, with S = 100,000 samples and L = 1,600,000 channel
/// samples, which is 56,324,288 bytes (53.7 MiB). A valid run never reaches it.
pub(super) const MAX_CHECKPOINT: usize = (MAX_RUN_SAMPLES
    * (SAMPLE_BYTES + FRAME_BYTES + TIME_BYTES)
    + MAX_CHANNEL_SAMPLES * VALUE_BYTES
    + HEADER_BYTES) as usize;

pub(super) struct Writer<W: Write = std::io::Stdout> {
    sink: W,
    bytes: usize,
    budget: usize,
}
impl Writer {
    /// Writes the header frame to the worker's standard output within the checkpoint budget.
    pub(super) fn new(header: &Value) -> Result<Self, String> {
        Self::with_sink(std::io::stdout(), header, MAX_CHECKPOINT)
    }
}
impl<W: Write> Writer<W> {
    fn with_sink(sink: W, header: &Value, budget: usize) -> Result<Self, String> {
        let mut writer = Self {
            sink,
            bytes: 0,
            budget,
        };
        writer.emit(HEADER, header)?;
        Ok(writer)
    }
    pub(super) fn sample(&mut self, row: &Value) -> Result<(), String> {
        self.emit(SAMPLE, row)
    }
    fn emit(&mut self, prefix: &str, value: &Value) -> Result<(), String> {
        let frame = format!(
            "{prefix}{}\n",
            serde_json::to_string(value).map_err(|e| e.to_string())?
        );
        if self.bytes.saturating_add(frame.len()) > self.budget {
            return Err("worker checkpoint output budget exhausted".into());
        }
        self.sink
            .write_all(frame.as_bytes())
            .and_then(|()| self.sink.flush())
            .map_err(|e| e.to_string())?;
        self.bytes += frame.len();
        Ok(())
    }
}

/// One wire sample: `readings` holds each channel's value (`None` when it has none) and its quality
/// code, in channel order. `quality` is omitted when every code is `good`.
pub(super) fn sample_row(
    sequence: u64,
    time_seconds: f64,
    readings: &[(Option<f64>, u64)],
) -> Value {
    let values: Vec<Value> = readings
        .iter()
        .map(|(value, _)| value.map_or(Value::Null, |number| json!(number)))
        .collect();
    let mut row = json!({"sequence": sequence, "timeSeconds": time_seconds, "values": values});
    if readings.iter().any(|(_, code)| *code != QUALITY_GOOD) {
        row["quality"] = readings
            .iter()
            .map(|(_, code)| Value::from(*code))
            .collect();
    }
    row
}

pub(super) struct WorkerOutput {
    pub result: Result<Value, String>,
    pub prefix: Option<Value>,
}

pub(super) fn read_output(reader: impl Read, captured: &AtomicU64) -> WorkerOutput {
    read_bounded(reader, captured, MAX_CHECKPOINT)
}

/// Reads a worker's stream and keeps the received prefix. `checkpoint_budget` bounds its frames.
fn read_bounded(reader: impl Read, captured: &AtomicU64, checkpoint_budget: usize) -> WorkerOutput {
    let mut reader = BufReader::new(reader.take(super::worker::MAX_OUTPUT + 1));
    let mut header: Option<Value> = None;
    let mut rows = Vec::new();
    let mut bytes = 0_usize;
    let mut checkpoint_bytes = 0_usize;
    let mut result: Result<Value, String> = Err("worker returned no typed result".into());
    let mut invalid = None;
    loop {
        let mut line = Vec::new();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Err(error) => {
                invalid = Some(error.to_string());
                break;
            }
            Ok(_) => {}
        }
        bytes = bytes.saturating_add(line.len());
        if bytes as u64 > super::worker::MAX_OUTPUT {
            invalid = Some("worker output exceeded bound".into());
            break;
        }
        // A killed writer can leave an incomplete last frame, including partial UTF-8.
        if !line.ends_with(b"\n") {
            break;
        }
        let Ok(text) = std::str::from_utf8(&line) else {
            invalid = Some("invalid worker output encoding".into());
            break;
        };
        if let Some(payload) = text.strip_prefix(super::worker::PREFIX) {
            result = serde_json::from_str(payload).map_err(|e| e.to_string());
            continue;
        }
        let is_header = text.starts_with(HEADER);
        let payload = text
            .strip_prefix(HEADER)
            .or_else(|| text.strip_prefix(SAMPLE));
        let Some(payload) = payload else {
            continue;
        };
        checkpoint_bytes = checkpoint_bytes.saturating_add(line.len());
        if checkpoint_bytes > checkpoint_budget {
            invalid = Some("worker checkpoint output exceeded bound".into());
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            invalid = Some("invalid worker checkpoint frame".into());
            break;
        };
        if is_header {
            if header.is_some() || !valid_header(&value) {
                invalid = Some("invalid worker checkpoint header".into());
                break;
            }
            header = Some(value);
        } else {
            if !header
                .as_ref()
                .is_some_and(|h| valid_sample(h, &value, rows.len()))
            {
                invalid = Some("invalid worker checkpoint sample".into());
                break;
            }
            rows.push(value);
            captured.store(rows.len() as u64, Ordering::SeqCst);
        }
    }
    if let Some(reason) = invalid {
        result = Err(reason);
    }
    // A streamed run's final result repeats the received header with no samples. The samples
    // come only from the frames, so the supervisor assembles the complete run here.
    let streamed = header.is_some();
    let samples = Value::Array(rows);
    let has_samples = samples.as_array().is_some_and(|rows| !rows.is_empty());
    let final_run_agrees = match (&header, &result) {
        (Some(header), Ok(final_result)) => match final_result.get("run") {
            None => !has_samples,
            Some(final_run) => has_samples && final_run == header,
        },
        _ => true,
    };
    let prefix = header.filter(|_| has_samples).map(|mut run| {
        run["samples"] = samples.clone();
        run
    });
    if streamed && !final_run_agrees {
        result = Err("final worker result disagrees with captured samples".into());
    } else if streamed {
        if let Ok(final_result) = &mut result {
            if let Some(final_run) = final_result.get_mut("run") {
                final_run["samples"] = samples;
            }
        }
    }
    WorkerOutput { result, prefix }
}

fn valid_header(run: &Value) -> bool {
    run["evidence"] == "ST-executed"
        && run["id"].as_str().is_some_and(|id| !id.is_empty())
        && run["parameters"].is_object()
        && run["conditions"].is_object()
        && run["samples"].as_array().is_some_and(Vec::is_empty)
        && run["conditions"]["sampleTimesSeconds"]
            .as_array()
            .is_some_and(|times| {
                // A record period's time is the mean of its grid times (A4.2): the first may be later
                // than zero. Times stay finite, non-negative and strictly increasing.
                !times.is_empty()
                    && times.len() <= super::execution_settings::MAX_RUN_SAMPLES as usize
                    && times.iter().enumerate().all(|(index, time)| {
                        time.as_f64().is_some_and(|time| {
                            time.is_finite()
                                && time >= 0.0
                                && (index == 0
                                    || times[index - 1]
                                        .as_f64()
                                        .is_some_and(|previous| time > previous))
                        })
                    })
            })
        && run["channels"].as_array().is_some_and(|channels| {
            !channels.is_empty()
                && channels.len() <= super::execution_settings::MAX_OBSERVATIONS
                && channels.iter().enumerate().all(|(index, channel)| {
                    channel["id"].as_str().is_some_and(|id| !id.is_empty())
                        && channel["unit"].is_string()
                        && !channels[..index]
                            .iter()
                            .any(|prior| prior["id"] == channel["id"])
                })
        })
}

/// A compact sample of the header's channels at its planned grid point (see the module comment).
/// A `null` value must carry a quality other than `good`.
fn valid_sample(header: &Value, row: &Value, index: usize) -> bool {
    let channels = header["channels"]
        .as_array()
        .expect("validated header")
        .len();
    let Some(values) = row["values"]
        .as_array()
        .filter(|values| values.len() == channels)
    else {
        return false;
    };
    let codes = match row.get("quality") {
        None => None,
        Some(codes) => match codes.as_array() {
            Some(codes) if codes.len() == channels => Some(codes),
            _ => return false,
        },
    };
    let code_count = QUALITY_NAMES.len() as u64;
    index < super::execution_settings::MAX_RUN_SAMPLES as usize
        && row["sequence"].as_u64() == Some(index as u64)
        && row["timeSeconds"].as_f64().is_some()
        && header["conditions"]["sampleTimesSeconds"].get(index) == Some(&row["timeSeconds"])
        && values.iter().enumerate().all(|(channel, value)| {
            let code = codes.map_or(Some(QUALITY_GOOD), |codes| codes[channel].as_u64());
            match (value, code) {
                (Value::Null, Some(code)) => code != QUALITY_GOOD && code < code_count,
                (Value::Number(_), Some(code)) => code < code_count,
                _ => false,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header() -> Value {
        json!({"id":"run-0","evidence":"ST-executed","parameters":{},
            "conditions":{"sampleTimesSeconds":[0.0,0.1]},"channels":[{"id":"PV","unit":"m"}],"samples":[]})
    }
    fn sample() -> Value {
        sample_row(0, 0.0, &[(Some(2.0), QUALITY_GOOD)])
    }
    fn frames(last: &str) -> String {
        format!("{HEADER}{}\n{SAMPLE}{}\n{last}", header(), sample())
    }
    #[test]
    fn checkpoint_accepts_all_64_channels_permitted_by_the_run_contract() {
        let mut run = header();
        run["channels"] = json!((0..64)
            .map(|index| json!({"id":format!("state-{index}"),"unit":"1"}))
            .collect::<Vec<_>>());
        let readings = |base: f64| {
            (0..64)
                .map(|index| (Some(base + index as f64), QUALITY_GOOD))
                .collect::<Vec<_>>()
        };
        let row = sample_row(0, 0.0, &readings(0.0));
        let second = sample_row(1, 0.1, &readings(100.0));
        let final_result = json!({"status":"executed","run":run});
        let stream = format!(
            "{HEADER}{run}\n{SAMPLE}{row}\n{SAMPLE}{second}\n{}{final_result}\n",
            super::super::worker::PREFIX
        );
        let captured = AtomicU64::new(0);
        let output = read_output(stream.as_bytes(), &captured);
        let result = output
            .result
            .expect("accepted channel count must survive checkpoint parsing");
        assert_eq!(result["status"], "executed");
        assert_eq!(result["run"]["samples"], json!([row, second]));
        assert_eq!(captured.load(Ordering::SeqCst), 2);
        run["channels"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"excess","unit":"1"}));
        let rejected = read_output(format!("{HEADER}{run}\n").as_bytes(), &AtomicU64::new(0));
        assert!(
            rejected.result.is_err(),
            "the declared channel budget remains enforced"
        );
    }

    #[test]
    fn duplicate_channels_cannot_admit_unrelated_values() {
        let captured = AtomicU64::new(0);
        let mut invalid = header();
        invalid["channels"] = json!([{"id":"PV","unit":"m"},{"id":"PV","unit":"m"}]);
        let mut row = sample();
        row["values"] = json!([2.0, 999.0]);
        let stream = format!("{HEADER}{invalid}\n{SAMPLE}{row}\n");
        let output = read_output(stream.as_bytes(), &captured);
        assert!(
            output.prefix.is_none(),
            "A malformed channel map cannot supply a genuine recorded prefix"
        );
        assert_eq!(captured.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn malformed_planned_time_grids_cannot_supply_a_prefix() {
        for times in [json!([0.0, -0.1]), json!([0.0, 0.0]), json!([0.0, null])] {
            let captured = AtomicU64::new(0);
            let mut invalid = header();
            invalid["conditions"]["sampleTimesSeconds"] = times;
            let stream = format!("{HEADER}{invalid}\n{SAMPLE}{}\n", sample());
            let output = read_output(stream.as_bytes(), &captured);
            assert!(
                output.prefix.is_none(),
                "A malformed planned grid cannot supply a genuine recorded prefix"
            );
        }
    }
    #[test]
    fn truncated_last_frame_preserves_only_complete_received_samples() {
        let captured = AtomicU64::new(0);
        let stream = frames("TRUST_RUN_SAMPLE {\"sequence\":1,");
        let output = read_output(stream.as_bytes(), &captured);
        assert!(output.result.is_err());
        assert_eq!(captured.load(Ordering::SeqCst), 1);
        assert_eq!(output.prefix.unwrap()["samples"], json!([sample()]));
    }
    #[test]
    fn inconsistent_final_run_cannot_override_the_received_samples() {
        let captured = AtomicU64::new(0);
        let mut different = header();
        different["samples"] = json!([sample()]);
        different["samples"][0]["values"][0] = json!(999.0);
        let stream = frames(&format!(
            "{}{}\n",
            super::super::worker::PREFIX,
            json!({"status":"executed","run":different})
        ));
        let output = read_output(stream.as_bytes(), &captured);
        assert!(
            output.result.is_err(),
            "A final result cannot replace already received actual sample values"
        );
        assert_eq!(output.prefix.unwrap()["samples"][0]["values"][0], 2.0);
    }
    #[test]
    fn a_final_result_that_repeats_the_frames_samples_is_refused() {
        let mut repeated = header();
        repeated["samples"] = json!([sample()]);
        let stream = frames(&format!(
            "{}{}\n",
            super::super::worker::PREFIX,
            json!({"status":"executed","run":repeated})
        ));
        let output = read_output(stream.as_bytes(), &AtomicU64::new(0));
        assert!(
            output.result.is_err(),
            "the frames are the only source of samples; a final result cannot repeat them"
        );
    }
    #[test]
    fn out_of_sequence_frames_are_rejected_without_synthesizing_a_gap() {
        let captured = AtomicU64::new(0);
        let skipped = sample_row(2, 0.2, &[(Some(2.0), QUALITY_GOOD)]);
        let stream = frames(&format!("{SAMPLE}{skipped}\n"));
        let output = read_output(stream.as_bytes(), &captured);
        assert!(output.result.is_err());
        assert_eq!(captured.load(Ordering::SeqCst), 1);
        assert_eq!(
            output.prefix.unwrap()["samples"].as_array().unwrap().len(),
            1
        );
    }
    #[test]
    fn compact_samples_round_trip_with_nulls_and_non_good_qualities() {
        let run = json!({"id":"run-0","evidence":"ST-executed","parameters":{},
            "conditions":{"sampleTimesSeconds":[0.0,0.1]},
            "channels":[{"id":"A","unit":"m"},{"id":"B","unit":"m"},{"id":"C","unit":"m"}],"samples":[]});
        let first = sample_row(
            0,
            0.0,
            &[
                (Some(1.5), QUALITY_GOOD),
                (None, 2),
                (Some(-2.0), QUALITY_BAD),
            ],
        );
        let second = sample_row(
            1,
            0.1,
            &[
                (None, 3),
                (Some(3.0), QUALITY_GOOD),
                (Some(4.0), QUALITY_GOOD),
            ],
        );
        assert_eq!(first["values"], json!([1.5, null, -2.0]));
        assert_eq!(first["quality"], json!([0, 2, 1]));
        assert_eq!(second["quality"], json!([3, 0, 0]));
        let stream = format!(
            "{HEADER}{run}\n{SAMPLE}{first}\n{SAMPLE}{second}\n{}{}\n",
            super::super::worker::PREFIX,
            json!({"status":"executed","run":run})
        );
        let captured = AtomicU64::new(0);
        let output = read_output(stream.as_bytes(), &captured);
        let result = output
            .result
            .expect("frames with nulls and non-good qualities are accepted");
        let mut expected = run.clone();
        expected["samples"] = json!([first, second]);
        assert_eq!(
            result["run"], expected,
            "the supervisor assembles the complete run from its frames"
        );
        assert_eq!(output.prefix, Some(expected));
        assert_eq!(captured.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn a_null_value_must_carry_a_non_good_quality() {
        let captured = AtomicU64::new(0);
        let stream = format!(
            "{HEADER}{}\n{SAMPLE}{}\n",
            header(),
            sample_row(0, 0.0, &[(None, QUALITY_GOOD)])
        );
        let output = read_output(stream.as_bytes(), &captured);
        assert!(output.result.is_err());
        assert!(output.prefix.is_none());
        assert_eq!(captured.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn quality_codes_outside_the_vocabulary_are_refused() {
        let captured = AtomicU64::new(0);
        let stream = format!(
            "{HEADER}{}\n{SAMPLE}{}\n",
            header(),
            sample_row(0, 0.0, &[(Some(2.0), QUALITY_NAMES.len() as u64)])
        );
        let output = read_output(stream.as_bytes(), &captured);
        assert!(output.result.is_err());
        assert_eq!(captured.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn checkpoint_writer_refuses_frames_beyond_its_budget() {
        let header = header();
        let header_len = format!("{HEADER}{header}\n").len();
        let frame_len = format!("{SAMPLE}{}\n", sample()).len();
        let mut writer = Writer::with_sink(Vec::new(), &header, header_len + frame_len)
            .expect("the header fits its budget");
        writer
            .sample(&sample())
            .expect("the first sample fits its budget");
        let refused = writer
            .sample(&sample())
            .expect_err("the second sample exceeds the budget");
        assert_eq!(refused, "worker checkpoint output budget exhausted");
        assert_eq!(writer.sink.len(), header_len + frame_len);
    }
    #[test]
    fn reader_keeps_the_received_prefix_when_the_checkpoint_budget_is_exceeded() {
        let times = [0.0, 0.1, 0.2];
        let mut planned = header();
        planned["conditions"]["sampleTimesSeconds"] = json!(times);
        let rows: Vec<Value> = times
            .iter()
            .enumerate()
            .map(|(index, time)| sample_row(index as u64, *time, &[(Some(2.0), QUALITY_GOOD)]))
            .collect();
        let header_len = format!("{HEADER}{planned}\n").len();
        let frame_len = format!("{SAMPLE}{}\n", rows[0]).len();
        let stream = format!(
            "{HEADER}{planned}\n{}",
            rows.iter()
                .map(|row| format!("{SAMPLE}{row}\n"))
                .collect::<String>()
        );
        let captured = AtomicU64::new(0);
        let output = read_bounded(stream.as_bytes(), &captured, header_len + 2 * frame_len);
        assert_eq!(
            output.result.unwrap_err(),
            "worker checkpoint output exceeded bound"
        );
        assert_eq!(captured.load(Ordering::SeqCst), 2);
        assert_eq!(output.prefix.unwrap()["samples"], json!([rows[0], rows[1]]));
    }
}
