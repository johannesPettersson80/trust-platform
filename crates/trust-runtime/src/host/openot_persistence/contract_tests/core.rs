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
