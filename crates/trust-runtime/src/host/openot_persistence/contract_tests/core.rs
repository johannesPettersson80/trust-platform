use super::*;
#[test]
fn sink_checkpoint_contract_reads_none_then_committed_cursor() {
    let mut sink = InMemoryDocumentSink::new();
    assert_eq!(
        sink.load_checkpoint(9, 1).expect("load empty checkpoint"),
        None
    );
    let batch = PersistenceBatch {
        documents: Vec::new(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 9,
            run_id: 1,
            cursor_abs: 123,
        },
    };
    sink.commit(&batch).expect("commit checkpoint");
    assert_eq!(
        sink.load_checkpoint(9, 1)
            .expect("load committed checkpoint"),
        Some(batch.checkpoint)
    );
    assert_eq!(
        sink.load_checkpoint(10, 1).expect("load other buffer"),
        None
    );
}
#[test]
fn in_memory_sink_commits_event_loss_placeholder_and_checkpoint_unchanged() {
    let documents = canonical_documents();
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 4096,
    };
    let batch = PersistenceBatch {
        documents: documents.clone(),
        checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();

    let outcome = sink.commit(&batch).expect("batch should commit atomically");

    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(checkpoint));
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.unresolved_documents, 1);
    assert_eq!(outcome.loss_ranges, 1);
    assert_eq!(outcome.lost_records, 2);
    assert_eq!(outcome.checkpoint, checkpoint);
}

#[test]
fn failed_sink_commit_advances_neither_documents_nor_checkpoint() {
    let original_checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 100,
    };
    let original_batch = PersistenceBatch {
        documents: vec![canonical_documents().remove(0)],
        checkpoint: original_checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();
    sink.commit(&original_batch).expect("baseline commit");
    let original_documents = sink.documents.clone();

    sink.fail_next_commit();
    let failed_batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 200,
        },
    };
    let result = sink.commit(&failed_batch);

    assert!(
        result.is_err(),
        "injected transaction failure must be visible"
    );
    assert_eq!(sink.documents, original_documents);
    assert_eq!(sink.checkpoint, Some(original_checkpoint));
}

#[test]
fn retrying_identical_batch_is_idempotent() {
    let documents = canonical_documents();
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 4096,
    };
    let batch = PersistenceBatch {
        documents: documents.clone(),
        checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();
    sink.commit(&batch).expect("first commit");

    let retried = sink.commit(&batch).expect("idempotent retry");

    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(checkpoint));
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(retried.unresolved_documents, 0);
    assert_eq!(retried.loss_ranges, 0);
    assert_eq!(retried.lost_records, 0);
}

#[test]
fn sink_rejects_checkpoint_regression_without_changing_durable_state() {
    let documents = canonical_documents();
    let mut sink = InMemoryDocumentSink::new();
    let committed = PersistenceBatch {
        documents: documents.clone(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 200,
        },
    };
    sink.commit(&committed).expect("baseline commit");

    let stale = PersistenceBatch {
        documents: Vec::new(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 100,
        },
    };
    let result = sink.commit(&stale);

    assert!(result.is_err(), "a stale cursor must fail closed");
    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(committed.checkpoint));
}

#[test]
fn influx_logged_values_preserve_previous_and_audit_fields() {
    let projector = super::super::projection::LoggingProjector::new(vec![
        open_ot_definition::sample_definition(),
    ])
    .expect("projector");
    let document = canonical_documents()
        .into_iter()
        .find(|document| {
            matches!(document, Document::Event(event) if event.event_type_id == open_ot_carriage::registry::EVENT_PARAMETER_CHANGE)
        })
        .expect("parameter-change fixture");
    let mut projected = projector.project(&document).expect("project document");
    let value = projected
        .logged_values
        .first_mut()
        .expect("logged value projection");
    // Exercise every optional lane in one encoder contract. A single OpenOT
    // value uses exactly one typed lane, so no real event can populate all of
    // these simultaneously.
    value.previous_boolean_value = Some(false);
    value.previous_signed_value = Some(-17);
    value.previous_unsigned_value = Some(u64::MAX.to_string());
    value.previous_number_value = Some(12.5);
    value.previous_text_value = Some("previous text".into());
    value.previous_exact_value = Some("previous exact".into());
    let value_lines = super::super::influxdb3_read_model::line_protocol(&projected)
        .expect("serialize Influx line protocol");

    for field in [
        "previous_boolean_value=",
        "previous_signed_value=",
        "previous_unsigned_value=",
        "previous_number_value=",
        "previous_text_value=",
        "previous_exact_value=",
        "actor=",
        "reason=",
        "authorization_result=",
    ] {
        assert!(
            value_lines.contains(field),
            "Influx logged_values must retain {field}: {value_lines}"
        );
    }
}

#[test]
fn influx_domain_measurements_preserve_the_complete_backend_neutral_rows() {
    use super::super::projection_domains::DomainRow;

    let projector = super::super::projection::LoggingProjector::new(vec![
        open_ot_definition::sample_definition(),
    ])
    .expect("projector");
    let mut lines = String::new();
    for document in canonical_documents() {
        let mut projected = projector.project(&document).expect("project document");
        for domain in &mut projected.domains {
            match domain {
                DomainRow::State(row) => {
                    row.previous_state_label = Some("Idle label".into());
                    row.new_state_label = Some("Running label".into());
                }
                DomainRow::Batch(row) => {
                    row.recipe_id = Some("Recipe-1".into());
                    row.previous_state = Some("Idle".into());
                    row.new_state_label = Some("Running label".into());
                }
                DomainRow::Recipe(row) => {
                    row.recipe_version = Some("2".into());
                    row.batch_id = Some("Batch-1".into());
                    row.actor = Some("operator-a".into());
                    row.authorization_result = Some("authorized".into());
                }
                DomainRow::Material(row) => row.unit = Some("kg".into()),
                DomainRow::Operator(row) => {
                    row.action_id = Some("Action-1".into());
                    row.actor = Some("operator-a".into());
                    row.workstation = Some("station-1".into());
                    row.role = Some("operator".into());
                    row.authorization_result = Some("authorized".into());
                    row.reason = Some("approved".into());
                    row.context_references = Some("Batch-1".into());
                }
                DomainRow::Audit(row) => {
                    row.authorization_result = Some("authorized".into());
                    row.workstation = Some("station-1".into());
                }
                DomainRow::Signature(row) => {
                    row.authorization_result = Some("authorized".into());
                }
                DomainRow::Unresolved(row) => {
                    row.source = Some("UnitA".into());
                    row.diagnostic_summary = Some("unknown event".into());
                }
                DomainRow::Loss(row) => row.source = Some("UnitA".into()),
                _ => {}
            }
        }
        lines.push_str(
            &super::super::influxdb3_read_model::line_protocol(&projected)
                .expect("serialize Influx line protocol"),
        );
        lines.push('\n');
    }

    for (measurement, fields) in [
        (
            "state_history,",
            &["previous_state_label=", "new_state_label="][..],
        ),
        (
            "batch_history,",
            &["recipe_id=", "previous_state=", "new_state_label="],
        ),
        (
            "recipe_history,",
            &[
                "recipe_version=",
                "batch_id=",
                "actor=",
                "authorization_result=",
            ],
        ),
        ("material_additions,", &["unit="][..]),
        (
            "operator_activity,",
            &[
                "action_id=",
                "actor=",
                "workstation=",
                "role=",
                "authorization_result=",
                "reason=",
                "context_references=",
            ],
        ),
        ("audit_log,", &["authorization_result=", "workstation="][..]),
        ("electronic_signatures,", &["authorization_result="][..]),
        (
            "data_loss,",
            &[
                "received_time=",
                "received_time_ns=",
                "source=",
                "buffer_id=",
                "epoch_id=",
                ",sequence=",
                "definition_hash=",
            ],
        ),
        (
            "unresolved_records,",
            &[
                "event_time=",
                "event_time_ns=",
                "received_time=",
                "received_time_ns=",
                "source=",
                "buffer_id=",
                "epoch_id=",
                "sequence=",
                "definition_hash=",
                "diagnostic_summary=",
            ],
        ),
    ] {
        let line = lines
            .lines()
            .find(|line| line.starts_with(measurement))
            .unwrap_or_else(|| panic!("missing {measurement} measurement"));
        for field in fields {
            assert!(
                line.contains(field),
                "{measurement} must retain {field}: {line}"
            );
        }
    }
    let event_line = lines
        .lines()
        .find(|line| line.starts_with("event_log,"))
        .expect("event-log measurement");
    for field in [
        "event_time=",
        "event_time_ns=",
        "received_time=",
        "received_time_ns=",
    ] {
        assert!(
            event_line.contains(field),
            "Influx event rows must retain named common time field {field}: {event_line}"
        );
    }
}
