//! Bounded private sample frames between an isolated worker and its supervisor.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

const HEADER: &str = "TRUST_RUN_HEADER ";
const SAMPLE: &str = "TRUST_RUN_SAMPLE ";
const MAX_CHECKPOINT: usize = 15 * 1024 * 1024;

pub(super) struct Writer {
    bytes: usize,
}
impl Writer {
    pub(super) fn new(header: &Value) -> Result<Self, String> {
        let mut writer = Self { bytes: 0 };
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
        if self.bytes.saturating_add(frame.len()) > MAX_CHECKPOINT {
            return Err("worker checkpoint output budget exhausted".into());
        }
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(frame.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|e| e.to_string())?;
        self.bytes += frame.len();
        Ok(())
    }
}

pub(super) struct WorkerOutput {
    pub result: Result<Value, String>,
    pub prefix: Option<Value>,
}

pub(super) fn read_output(reader: impl Read, captured: &AtomicU64) -> WorkerOutput {
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
        if checkpoint_bytes > MAX_CHECKPOINT {
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
    let had_header = header.is_some();
    let prefix = header.filter(|_| !rows.is_empty()).map(|mut run| {
        run["samples"] = json!(rows);
        run
    });
    if had_header
        && result
            .as_ref()
            .is_ok_and(|final_result| final_result.get("run") != prefix.as_ref())
    {
        result = Err("final worker result disagrees with captured samples".into());
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
                !times.is_empty()
                    && times.len() <= 12000
                    && times.iter().enumerate().all(|(index, time)| {
                        time.as_f64().is_some_and(|time| {
                            time.is_finite()
                                && if index == 0 {
                                    time == 0.0
                                } else {
                                    times[index - 1]
                                        .as_f64()
                                        .is_some_and(|previous| time > previous)
                                }
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
fn valid_sample(header: &Value, row: &Value, index: usize) -> bool {
    let channels = header["channels"].as_array().expect("validated header");
    let Some(values) = row["values"].as_object() else {
        return false;
    };
    index < 12000
        && row["sequence"].as_u64() == Some(index as u64)
        && row["timeSeconds"].as_f64().is_some()
        && header["conditions"]["sampleTimesSeconds"].get(index) == Some(&row["timeSeconds"])
        && values.len() == channels.len()
        && channels.iter().all(|channel| {
            values
                .get(channel["id"].as_str().expect("validated header"))
                .is_some_and(|value| {
                    value["quality"] == "good"
                        && value["value"].as_f64().is_some_and(f64::is_finite)
                })
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
        json!({"sequence":0,"timeSeconds":0.0,"values":{"PV":{"value":2.0,"quality":"good"}}})
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
        let mut row = sample();
        row["values"] = Value::Object(
            (0..64)
                .map(|index| {
                    (
                        format!("state-{index}"),
                        json!({"value":index,"quality":"good"}),
                    )
                })
                .collect(),
        );
        let mut second = row.clone();
        second["sequence"] = json!(1);
        second["timeSeconds"] = json!(0.1);
        let mut final_run = run.clone();
        final_run["samples"] = json!([row.clone(), second.clone()]);
        let final_result = json!({"status":"executed","run":final_run});
        let stream = format!(
            "{HEADER}{run}\n{SAMPLE}{row}\n{SAMPLE}{second}\n{}{final_result}\n",
            super::super::worker::PREFIX
        );
        let captured = AtomicU64::new(0);
        let output = read_output(stream.as_bytes(), &captured);
        assert_eq!(
            output
                .result
                .expect("accepted channel count must survive checkpoint parsing"),
            final_result
        );
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
        row["values"]["unrelated"] = json!({"value":999.0,"quality":"good"});
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
        different["samples"][0]["values"]["PV"]["value"] = json!(999.0);
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
        assert_eq!(
            output.prefix.unwrap()["samples"][0]["values"]["PV"]["value"],
            2.0
        );
    }
    #[test]
    fn out_of_sequence_frames_are_rejected_without_synthesizing_a_gap() {
        let captured = AtomicU64::new(0);
        let mut skipped = sample();
        skipped["sequence"] = json!(2);
        skipped["timeSeconds"] = json!(0.2);
        let stream = frames(&format!("{SAMPLE}{skipped}\n"));
        let output = read_output(stream.as_bytes(), &captured);
        assert!(output.result.is_err());
        assert_eq!(captured.load(Ordering::SeqCst), 1);
        assert_eq!(
            output.prefix.unwrap()["samples"].as_array().unwrap().len(),
            1
        );
    }
}
