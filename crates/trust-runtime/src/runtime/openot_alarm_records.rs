//! The alarm service's OpenOT records (specification 40, §5.2): each alarm transition as a
//! condition record whose identity is the alarm's instance path.

use open_ot_carriage::registry::{
    EVENT_CONDITION_ACKNOWLEDGED, EVENT_CONDITION_ACTIVE, EVENT_CONDITION_CLEARED,
    EVENT_CONDITION_COMMENTED, EVENT_CONDITION_IN_SERVICE, EVENT_CONDITION_OUT_OF_SERVICE,
    EVENT_CONDITION_RESET, EVENT_CONDITION_SHELVED, EVENT_CONDITION_SUPPRESSED,
    EVENT_CONDITION_UNSHELVED, EVENT_CONDITION_UNSUPPRESSED, KEY_ACK_BY, KEY_COMMENT,
    KEY_CONDITION_ID, KEY_CORRELATION_ID, KEY_SEVERITY, KEY_SHELVE_SECS, KEY_SOURCE_PATH,
    TY_STRING, TY_UDINT, TY_UINT,
};
use open_ot_carriage::wire::{Record, Slot, FLAG_PARTIAL_PAYLOAD};

use crate::hmi::alarms::{AlarmEvent, EventKind, Priority};

/// The alarm service's OpenOT source, apart from the identities authored programs receive.
pub(crate) const ALARM_SOURCE_ID: u32 = 0xF000;
/// The longest slot payload.
const SLOT_MAX: usize = 255;

/// The record of one transition, or none for a command's outcome, which is no condition record.
pub(crate) fn alarm_record(event: &AlarmEvent, run_id: u64, seq: u64) -> Option<Record> {
    let event_type = match event.kind {
        EventKind::Raised => EVENT_CONDITION_ACTIVE,
        EventKind::Cleared => EVENT_CONDITION_CLEARED,
        EventKind::Acknowledged => EVENT_CONDITION_ACKNOWLEDGED,
        EventKind::Shelved => EVENT_CONDITION_SHELVED,
        EventKind::Unshelved => EVENT_CONDITION_UNSHELVED,
        EventKind::Suppressed => EVENT_CONDITION_SUPPRESSED,
        EventKind::Unsuppressed => EVENT_CONDITION_UNSUPPRESSED,
        EventKind::OutOfService => EVENT_CONDITION_OUT_OF_SERVICE,
        EventKind::InService => EVENT_CONDITION_IN_SERVICE,
        EventKind::Comment => EVENT_CONDITION_COMMENTED,
        EventKind::Reset => EVENT_CONDITION_RESET,
        EventKind::Command => return None,
    };
    let mut record = Record::new(
        event.time_ms.saturating_mul(1_000_000),
        run_id,
        seq,
        ALARM_SOURCE_ID,
        event_type,
    );
    let mut partial = false;
    record.slots.push(Slot::new(
        KEY_CONDITION_ID,
        TY_UDINT,
        condition_id(&event.path).to_le_bytes().to_vec(),
    ));
    record
        .slots
        .push(text_slot(KEY_SOURCE_PATH, &event.path, &mut partial));
    if matches!(event.kind, EventKind::Raised | EventKind::Cleared) {
        if let Some(correlation) = event.correlation {
            record.slots.push(Slot::new(
                KEY_CORRELATION_ID,
                TY_UDINT,
                correlation.to_le_bytes().to_vec(),
            ));
        }
    }
    if event.kind == EventKind::Raised {
        let severity: u16 = match event.priority {
            Some(Priority::High) => 900,
            Some(Priority::Medium) => 500,
            _ => 100,
        };
        record.slots.push(Slot::new(
            KEY_SEVERITY,
            TY_UINT,
            severity.to_le_bytes().to_vec(),
        ));
    }
    if let Some(user) = &event.user {
        record.slots.push(text_slot(KEY_ACK_BY, user, &mut partial));
    }
    if let (EventKind::Shelved, Some(seconds)) = (event.kind, event.seconds) {
        record.slots.push(Slot::new(
            KEY_SHELVE_SECS,
            TY_UDINT,
            seconds.to_le_bytes().to_vec(),
        ));
    }
    if let (EventKind::Comment, Some(comment)) = (event.kind, &event.detail) {
        record
            .slots
            .push(text_slot(KEY_COMMENT, comment, &mut partial));
    }
    if partial {
        record.flags |= FLAG_PARTIAL_PAYLOAD;
    }
    Some(record)
}

/// The condition's number: the 32-bit FNV-1a hash of its path in lower case, 1 in place of 0,
/// which ConditionId may not be.
pub(crate) fn condition_id(path: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in path.to_ascii_lowercase().bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash.max(1)
}

/// A text slot, cut at a character boundary to the longest payload; `partial` notes a cut.
fn text_slot(key: u16, text: &str, partial: &mut bool) -> Slot {
    let mut end = text.len().min(SLOT_MAX);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    if end < text.len() {
        *partial = true;
    }
    Slot::new(key, TY_STRING, text.as_bytes()[..end].to_vec())
}
