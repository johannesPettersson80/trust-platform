fn update_alarm_state(state: &mut HmiLiveState, widget: &HmiWidgetSchema, value: f64, ts_ms: u128) {
    let violation = alarm_violation(value, widget.min, widget.max);
    let clear_window = alarm_clear_window(value, widget.min, widget.max, widget.alarm_deadband);
    let display_label = widget
        .alarm_label
        .as_ref()
        .filter(|label| !label.trim().is_empty())
        .unwrap_or(&widget.label)
        .clone();
    let mut raised = false;
    let mut cleared = false;
    let (id, widget_id, path, label) = {
        let alarm = state
            .alarms
            .entry(widget.id.clone())
            .or_insert_with(|| HmiAlarmState {
                id: widget.id.clone(),
                widget_id: widget.id.clone(),
                path: widget.path.clone(),
                label: display_label.clone(),
                active: false,
                acknowledged: false,
                raised_at_ms: 0,
                last_change_ms: 0,
                value,
                min: widget.min,
                max: widget.max,
            });
        alarm.value = value;
        alarm.min = widget.min;
        alarm.max = widget.max;
        alarm.label = display_label;
        if violation {
            if !alarm.active {
                alarm.active = true;
                alarm.acknowledged = false;
                alarm.raised_at_ms = ts_ms;
                alarm.last_change_ms = ts_ms;
                raised = true;
            }
        } else if alarm.active && clear_window {
            alarm.active = false;
            alarm.acknowledged = false;
            alarm.last_change_ms = ts_ms;
            cleared = true;
        }
        (
            alarm.id.clone(),
            alarm.widget_id.clone(),
            alarm.path.clone(),
            alarm.label.clone(),
        )
    };
    if raised {
        push_alarm_history(
            state,
            HmiAlarmHistoryRecord {
                id,
                widget_id,
                path,
                label,
                event: "raised",
                timestamp_ms: ts_ms,
                value,
            },
        );
    } else if cleared {
        push_alarm_history(
            state,
            HmiAlarmHistoryRecord {
                id,
                widget_id,
                path,
                label,
                event: "cleared",
                timestamp_ms: ts_ms,
                value,
            },
        );
    }
}

fn alarm_violation(value: f64, min: Option<f64>, max: Option<f64>) -> bool {
    if let Some(min) = min {
        if value < min {
            return true;
        }
    }
    if let Some(max) = max {
        if value > max {
            return true;
        }
    }
    false
}

fn alarm_clear_window(
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    deadband: Option<f64>,
) -> bool {
    let deadband = deadband.unwrap_or(0.0).max(0.0);
    if let Some(min) = min {
        if value < min + deadband {
            return false;
        }
    }
    if let Some(max) = max {
        if value > max - deadband {
            return false;
        }
    }
    true
}

fn push_alarm_history(state: &mut HmiLiveState, event: HmiAlarmHistoryRecord) {
    state.history.push_back(event);
    while state.history.len() > ALARM_HISTORY_LIMIT {
        let _ = state.history.pop_front();
    }
}

fn downsample_trend_samples(samples: &[HmiTrendSample], buckets: usize) -> Vec<HmiTrendPoint> {
    if samples.is_empty() {
        return Vec::new();
    }
    if samples.len() <= buckets {
        return samples
            .iter()
            .map(|sample| HmiTrendPoint {
                ts_ms: sample.ts_ms,
                value: sample.value,
                min: sample.value,
                max: sample.value,
                samples: 1,
            })
            .collect();
    }

    let chunk_size = samples.len().div_ceil(buckets);
    samples
        .chunks(chunk_size.max(1))
        .map(|chunk| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            let mut sum = 0.0;
            for sample in chunk {
                min = min.min(sample.value);
                max = max.max(sample.value);
                sum += sample.value;
            }
            HmiTrendPoint {
                ts_ms: chunk.last().map(|sample| sample.ts_ms).unwrap_or_default(),
                value: sum / chunk.len() as f64,
                min,
                max,
                samples: chunk.len(),
            }
        })
        .collect()
}

fn numeric_value_from_json(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::Bool(boolean) => Some(if *boolean { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn to_alarm_record(state: &HmiAlarmState) -> HmiAlarmRecord {
    HmiAlarmRecord {
        id: state.id.clone(),
        widget_id: state.widget_id.clone(),
        path: state.path.clone(),
        label: state.label.clone(),
        state: if state.acknowledged {
            "acknowledged"
        } else {
            "raised"
        },
        acknowledged: state.acknowledged,
        raised_at_ms: state.raised_at_ms,
        last_change_ms: state.last_change_ms,
        value: state.value,
        min: state.min,
        max: state.max,
    }
}
