//! What a status read does to a transfer loop, and the outcome that it reports (E6.3, E6.10).
use super::*;

/// The last outcome of a loop, as a status reports it.
pub(super) struct Outcome {
    pub(super) status: String,
    pub(super) reason: Option<String>,
    pub(super) transaction_id: Option<String>,
    pub(super) transaction: JsonValue,
}

impl TransferRegistry {
    /// Records the revisions that a status read reviewed (E6.10, E6.5 step 16). This marker is all that a status
    /// changes: it never creates, overwrites or resets a transaction record.
    pub(super) fn review(
        &mut self,
        loop_id: &str,
        binding_revision: &str,
        program_revision: &str,
    ) -> Result<(), String> {
        if !self.reviews.contains_key(loop_id) && self.reviews.len() >= MAX_LOOP_RECORDS {
            return Err(format!(
                "transfer loop-record capacity reached ({MAX_LOOP_RECORDS})"
            ));
        }
        self.epoch_revision = Some(program_revision.to_owned());
        self.reviews.insert(
            loop_id.to_owned(),
            Review {
                binding_revision: binding_revision.to_owned(),
                program_revision: program_revision.to_owned(),
            },
        );
        Ok(())
    }

    /// The last transaction of a loop, shown only for the binding that owns it. A transaction that a restart stopped
    /// keeps its reason, so status reports why it stopped (A6.7).
    pub(super) fn outcome(&self, loop_id: &str, binding_revision: &str) -> Outcome {
        let owned = self.records.get(loop_id).filter(|record| {
            record.transaction_binding_revision.as_deref() == Some(binding_revision)
        });
        match owned {
            Some(record) => Outcome {
                status: record.status.clone(),
                reason: record.reason.clone(),
                transaction_id: record.transaction_id.clone(),
                transaction: transaction_json(record),
            },
            None => Outcome {
                status: "idle".into(),
                reason: None,
                transaction_id: None,
                transaction: JsonValue::Null,
            },
        }
    }
}

fn transaction_json(record: &Record) -> JsonValue {
    record
        .transaction_id
        .as_ref()
        .map_or(JsonValue::Null, |id| {
            json!({
                "id": id,
                "program_revision": record.transaction_program_revision.clone(),
                "binding_revision": record.transaction_binding_revision.clone(),
                "status": record.status.clone(),
                "reason": record.reason.clone(),
                "observed_values": record.observed_values.clone(),
                "confirmation": record.confirmation.clone(),
                "initial_values": record.initial_values.clone(),
                "applied_values": record.applied_values.clone(),
                "restored": record.restored,
            })
        })
}
