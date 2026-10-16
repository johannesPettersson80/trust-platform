//! Qualified management queue publication preserves existing generation semantics.
use super::*;
use crate::ads::{AdsNotificationMode, AdsPointAddress};
use trust_ads_core::{PointAccess, UpdateMode};

#[test]
fn qualified_live_write_commits_one_generation_and_preserves_readonly_rejection() {
    let binding = AdsBinding {
        point: AdsPointConfig {
            point_name: "point".into(),
            address: AdsPointAddress::Symbol("Remote.Value".into()),
            data_type: AdsDataTypeDescriptor::scalar("INT", IecDataType::Int),
            access: PointAccess::ReadWrite,
            mode: UpdateMode::Poll,
            notification_mode: AdsNotificationMode::OnChange,
            allow_retain_read: false,
        },
        reference: ValueRef {
            location: crate::memory::MemoryLocation::Global,
            offset: 0,
            path: Vec::new(),
        },
        quality_reference: None,
        type_id: TypeId::INT,
        retain: false,
    };
    let mut bridge = AdsConnectionBridge::new(vec![binding]).unwrap();
    assert_eq!(bridge.live_value_writable("point"), Some(true));
    bridge.commit_qualified_live_write("point", Value::Int(3));
    let batch = bridge.shared.pending_write_batch();
    assert_eq!(batch.0.len(), 1);
    assert_eq!(batch.0["point"].value, Value::Int(3));
    let generation = batch.0["point"].generation;
    bridge.commit_qualified_live_write("point", Value::Int(4));
    assert_eq!(
        bridge.shared.pending_write_batch().0["point"].generation,
        generation + 1
    );
    bridge.bindings[0].point.access = PointAccess::Read;
    assert!(!bridge.queue_live_write("point", Value::Int(5)));
    let batch = bridge.shared.pending_write_batch();
    assert_eq!(batch.0["point"].generation, generation + 1);
    assert_eq!(batch.0["point"].value, Value::Int(4));
}
