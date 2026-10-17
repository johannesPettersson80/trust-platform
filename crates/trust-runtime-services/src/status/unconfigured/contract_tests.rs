//! The `status` blocks of the services a runtime does not run are the bytes trust-runtime wrote
//! for them before they became shared code: its `realtime` block for the default posture of
//! `LinuxRtConfig`, and its `openot_persistence` block without a persistence status.

use alloc::string::ToString;

use super::*;

#[test]
fn a_runtime_without_a_realtime_profile_reports_trust_runtimes_block_byte_for_byte() {
    assert_eq!(
        absent_realtime_profile().to_string(),
        concat!(
            r#"{"active":false,"enabled":false,"errors":[],"#,
            r#""observed":{"cpu_affinity":[],"kernel_realtime":null,"memory_locked_kb":null,"#,
            r#""priority":null,"scheduler":null},"#,
            r#""profile":"disabled","#,
            r#""requested":{"cpu_affinity":[],"lock_memory":false,"priority":70,"#,
            r#""scheduler":"other"},"#,
            r#""require_preempt_rt_kernel":false,"#,
            r#""runtime_applied":{"affinity":false,"memory_lock":false,"scheduler":false},"#,
            r#""strict":false,"warnings":[]}"#,
        )
    );
}

#[test]
fn a_runtime_without_openot_persistence_reports_trust_runtimes_block_byte_for_byte() {
    assert_eq!(
        disabled_openot_persistence().to_string(),
        concat!(
            r#"{"backend":null,"cursor_abs":0,"cursor_lag":0,"#,
            r#""documents_committed":0,"documents_duplicated":0,"documents_read":0,"#,
            r#""documents_retried":0,"head_abs":0,"last_error":null,"#,
            r#""last_success_time_ns":null,"loss_range_count":0,"lost_record_count":0,"#,
            r#""pending":0,"pending_part_count":0,"projection_rows_committed":0,"#,
            r#""reconciled_part_count":0,"rejected":0,"remote_pending":0,"#,
            r#""schema_generation":null,"state":"disabled","unclassified_event_count":0,"#,
            r#""unresolved":0,"warnings":[]}"#,
        )
    );
}
