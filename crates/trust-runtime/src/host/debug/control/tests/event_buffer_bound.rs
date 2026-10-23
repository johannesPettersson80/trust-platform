use super::super::*;
use crate::value::Duration;

fn write_rejected(index: usize) -> RuntimeEvent {
    RuntimeEvent::WriteRejected {
        target: format!("target{index}").into(),
        error: "out of range".into(),
        time: Duration::ZERO,
    }
}

#[test]
fn unattached_buffer_keeps_newest_1024_events_and_counts_the_dropped_oldest() {
    let control = DebugControl::new();
    for index in 0..=RUNTIME_EVENT_BUFFER_CAPACITY {
        control.push_runtime_event(write_rejected(index));
    }
    assert_eq!(control.runtime_events_dropped(), 1);

    let expected: Vec<RuntimeEvent> = (1..=RUNTIME_EVENT_BUFFER_CAPACITY)
        .map(write_rejected)
        .collect();
    assert_eq!(control.drain_runtime_events(), expected);
    assert_eq!(control.runtime_events_dropped(), 1);

    for index in 0..3 {
        control.push_runtime_event(write_rejected(RUNTIME_EVENT_BUFFER_CAPACITY + 1 + index));
    }
    assert_eq!(control.drain_runtime_events().len(), 3);
    assert_eq!(control.runtime_events_dropped(), 1);
}
