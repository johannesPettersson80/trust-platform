//! Tests of the Apply section (specification 35, A8.1, A8.4, A8.5, B6.4, B6.5 and B10.4): when Restore and Keep are offered from the
//! receipts, the rules a section keeps, and the refusals that name the missing Apply.

use serde_json::json;

use super::{
    keep_unavailable, restore_unavailable, ApplicationReceipt, ApplySection, KeepReceipt,
    RestorationReceipt,
};
use crate::commissioning::commissioning_result::CommissioningResult;
use crate::commissioning::found_mode::FoundMode;
use crate::failure::NextAction;
use crate::study::{Study, StudySection};

/// A stored result with the given status word (`passed`, `failed` or `cancelled`), as a study holds it.
fn result(status: &str) -> CommissioningResult {
    serde_json::from_value(json!({
        "status": status,
        "reason": "The response was assessed.",
        "transfers": [],
        "recoveryConfirmed": false,
        "target": 38.0,
        "modelRevision": "model:1",
        "channels": [],
        "criteria": {
            "target": 38.0,
            "durationSeconds": 60.0,
            "settlingTimeSeconds": 30.0,
            "maximumError": 0.2,
            "taskPeriodMs": 100.0,
            "processRange": { "minimum": 0.0, "maximum": 80.0 },
            "outputLimits": { "minimum": 0.0, "maximum": 100.0 }
        }
    }))
    .expect("a stored result")
}

/// The receipt of an Apply of `checkpoint` whose result has `status`, with its settings applied or not.
fn application(checkpoint: &str, status: &str, settings_applied: bool) -> ApplicationReceipt {
    ApplicationReceipt {
        checkpoint: checkpoint.to_string(),
        validation_identity: "validation:1".to_string(),
        settings_applied,
        result: result(status),
    }
}

/// The receipt of a Restore of `checkpoint`.
fn restoration(checkpoint: &str, restored: bool, retired: bool) -> RestorationReceipt {
    RestorationReceipt {
        checkpoint: checkpoint.to_string(),
        restored,
        retired,
        reason: "The previous settings were confirmed back.".to_string(),
        mode: Some(FoundMode::Auto),
    }
}

/// A section of the given Applies and Restores, and no Keep.
fn section(
    applications: Vec<ApplicationReceipt>,
    restorations: Vec<RestorationReceipt>,
) -> ApplySection {
    ApplySection {
        applications,
        restorations,
        keeps: Vec::new(),
    }
}

#[test]
fn a_section_without_an_apply_offers_neither_restore_nor_keep() {
    let empty = ApplySection::default();
    assert!(!empty.restore_available());
    assert!(!empty.keep_available());
    assert_eq!(empty.keep_checkpoint(), None);
}

#[test]
fn settled_settings_that_passed_or_failed_a_requirement_offer_restore_and_keep() {
    for status in ["passed", "failed"] {
        let settled = section(vec![application("cp-1", status, true)], Vec::new());
        assert!(settled.restore_available(), "{status}");
        assert!(
            settled.keep_available(),
            "{status}: Keep follows an Apply whose settings are applied (B6.5)"
        );
        assert_eq!(settled.keep_checkpoint(), Some("cp-1"));
    }
}

#[test]
fn a_cancelled_apply_whose_settings_stay_applied_offers_restore_but_not_keep() {
    let cancelled = section(vec![application("cp-1", "cancelled", true)], Vec::new());
    assert!(
        cancelled.restore_available(),
        "the settings are applied, so Restore returns them (B6.4)"
    );
    assert!(
        !cancelled.keep_available(),
        "Keep follows a passed result or a failed requirement only (B6.5)"
    );
}

#[test]
fn settings_that_were_never_applied_offer_nothing() {
    for status in ["passed", "failed", "cancelled"] {
        let refused = section(vec![application("cp-1", status, false)], Vec::new());
        assert!(!refused.restore_available(), "{status}");
        assert!(!refused.keep_available(), "{status}");
    }
}

#[test]
fn a_confirmed_restore_closes_its_checkpoint_for_restore_and_keep() {
    let restored = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", true, false)],
    );
    assert!(!restored.restore_available());
    assert!(
        !restored.keep_available(),
        "Keep is not offered after a Restore (B6.5)"
    );
}

#[test]
fn an_unconfirmed_restore_offers_restore_again_and_blocks_keep() {
    let unconfirmed = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", false, false)],
    );
    assert!(
        unconfirmed.restore_available(),
        "a Restore that is not confirmed is restored again (B6.7)"
    );
    assert!(
        !unconfirmed.keep_available(),
        "a Restore that is not confirmed blocks the dependent Keep (B6.7)"
    );
}

#[test]
fn a_retired_checkpoint_offers_neither_restore_nor_keep() {
    let retired = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", false, true)],
    );
    assert!(
        !retired.restore_available(),
        "a runtime that was replaced leaves no settings to return (B6.4)"
    );
    assert!(!retired.keep_available());
}

#[test]
fn the_newest_apply_decides_and_an_earlier_one_is_history() {
    let refused_later = section(
        vec![
            application("cp-1", "passed", true),
            application("cp-2", "failed", false),
        ],
        Vec::new(),
    );
    assert!(
        !refused_later.restore_available(),
        "a later Apply replaced the checkpoint of cp-1 (A8.5)"
    );
    assert!(!refused_later.keep_available());
    let applied_later = section(
        vec![
            application("cp-1", "passed", true),
            application("cp-2", "passed", true),
        ],
        vec![restoration("cp-1", true, false)],
    );
    assert!(
        applied_later.restore_available(),
        "restoring cp-1 does not close cp-2"
    );
    assert_eq!(applied_later.keep_checkpoint(), Some("cp-2"));
}

#[test]
fn a_kept_apply_is_not_kept_again_but_its_settings_still_restore() {
    let mut kept = section(vec![application("cp-1", "passed", true)], Vec::new());
    kept.keeps.push(KeepReceipt {
        checkpoint: "cp-1".to_string(),
        record: "Main.TIC101_Parameters".to_string(),
        kept: vec!["Kp".to_string()],
        unknown: Vec::new(),
    });
    assert!(
        !kept.keep_available(),
        "once Keep has kept these values, it is not offered again for them (control-studio-three-page-application)"
    );
    assert!(
        kept.restore_available(),
        "the settings are still applied, so Restore returns them"
    );
    kept.applications.push(application("cp-2", "passed", true));
    assert!(
        kept.keep_available(),
        "a later Apply offers Keep again for its own values"
    );
}

#[test]
fn every_receipt_names_its_checkpoint_and_a_broken_section_is_refused() {
    let unnamed = section(vec![application("", "passed", true)], Vec::new());
    assert!(unnamed
        .check()
        .expect_err("refused")
        .message
        .contains("names no checkpoint"));
    let duplicate = section(
        vec![
            application("cp-1", "passed", true),
            application("cp-1", "failed", true),
        ],
        Vec::new(),
    );
    assert!(duplicate
        .check()
        .expect_err("refused")
        .message
        .contains("same checkpoint"));
    let orphan_restore = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-2", true, false)],
    );
    assert!(orphan_restore
        .check()
        .expect_err("refused")
        .message
        .contains("checkpoint of no Apply"));
    let both = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", true, true)],
    );
    assert!(both
        .check()
        .expect_err("refused")
        .message
        .contains("both confirmed and retired"));
    let orphan_keep = ApplySection {
        applications: vec![application("cp-1", "passed", true)],
        restorations: Vec::new(),
        keeps: vec![KeepReceipt {
            checkpoint: "cp-9".to_string(),
            record: "Main.TIC101_Parameters".to_string(),
            kept: Vec::new(),
            unknown: Vec::new(),
        }],
    };
    assert!(orphan_keep
        .check()
        .expect_err("refused")
        .message
        .contains("checkpoint of no Apply"));
    let fine = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", false, false)],
    );
    assert!(fine.check().is_ok());
}

#[test]
fn the_section_is_kept_in_the_study_and_read_back_whole() {
    let mut study = Study::new("loop:tank", "sha256:abc");
    assert_eq!(study.section::<ApplySection>().expect("read"), None);
    let kept = section(
        vec![application("cp-1", "passed", true)],
        vec![restoration("cp-1", true, false)],
    );
    study.set_section(&kept).expect("kept");
    assert!(
        study.recordings.is_empty(),
        "a section whose results hold no recording lists none"
    );
    let text = study.encode().expect("encoded");
    let reopened = Study::decode(&text, "loop:tank").expect("decoded");
    assert_eq!(
        reopened.section::<ApplySection>().expect("read"),
        Some(kept.clone())
    );
    let document = kept.encode().expect("the section encodes");
    assert_eq!(
        ApplySection::decode(&document).expect("decoded"),
        kept,
        "the section's document reads back as the same section"
    );
}

#[test]
fn the_refusals_name_the_missing_apply_and_have_no_next_action() {
    let restore = restore_unavailable();
    assert!(
        restore
            .message
            .contains("Restore previous settings is not available"),
        "{}",
        restore.message
    );
    assert_eq!(restore.next_action, NextAction::None);
    let keep = keep_unavailable();
    assert!(
        keep.message.contains("Keep in project is not available"),
        "{}",
        keep.message
    );
    assert_eq!(keep.next_action, NextAction::None);
}
