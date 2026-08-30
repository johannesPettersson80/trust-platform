use super::projection::{EventLogRow, ProjectedDocument};
use super::projection_domains::DomainRow;
use super::PersistenceError;

pub(super) fn line_protocol(projected: &ProjectedDocument) -> Result<String, PersistenceError> {
    let mut lines = vec![super::influxdb3::canonical_line_protocol(
        &projected.canonical,
    )?];
    if let Some(event) = &projected.event {
        lines.push(event_line(
            "event_log",
            event,
            "event",
            vec![
                uint("event_type_id", event.event_type_id),
                string("event_name", &event.event_name),
                boolean("has_unclassified_fields", event.has_unclassified_fields),
            ],
        )?);
    }
    for value in &projected.logged_values {
        let mut fields = vec![
            uint("value_id", value.value_id),
            string("value_name", &value.value_name),
            string("value_type", value.value_type),
            uint("semantic_role", value.semantic_role),
            string("exact_value", &value.exact_value),
            boolean("is_audited", value.is_audited),
        ];
        optional_string(&mut fields, "unit", value.unit.as_deref());
        optional_uint(&mut fields, "quality", value.quality);
        optional_bool(&mut fields, "boolean_value", value.boolean_value);
        optional_signed(&mut fields, "signed_value", value.signed_value);
        optional_unsigned_text(
            &mut fields,
            "unsigned_value",
            value.unsigned_value.as_deref(),
        );
        optional_float(&mut fields, "number_value", value.number_value);
        optional_string(&mut fields, "text_value", value.text_value.as_deref());
        lines.push(event_line("logged_values", &value.common, "value", fields)?);
    }
    for domain in &projected.domains {
        lines.push(domain_line(domain)?);
    }
    Ok(lines.join("\n"))
}

fn domain_line(row: &DomainRow) -> Result<String, PersistenceError> {
    match row {
        DomainRow::Alarm(row) => {
            let mut fields = vec![
                string("condition", &row.condition),
                string("lifecycle_action", &row.lifecycle_action),
            ];
            optional_string(
                &mut fields,
                "condition_class",
                row.condition_class.as_deref(),
            );
            optional_string(&mut fields, "correlation_id", row.correlation_id.as_deref());
            optional_uint(&mut fields, "severity", row.severity);
            optional_string(&mut fields, "severity_label", row.severity_label.as_deref());
            optional_string(&mut fields, "cause", row.cause.as_deref());
            optional_string(&mut fields, "actor", row.actor.as_deref());
            optional_string(&mut fields, "reason", row.reason.as_deref());
            optional_string(&mut fields, "comment", row.comment.as_deref());
            optional_uint(&mut fields, "shelve_seconds", row.shelve_seconds);
            optional_uint(&mut fields, "previous_priority", row.previous_priority);
            optional_uint(&mut fields, "new_priority", row.new_priority);
            event_line("alarm_history", &row.common, "alarm", fields)
        }
        DomainRow::Message(row) => {
            let mut fields = vec![string("message_template", &row.message_template)];
            optional_uint(&mut fields, "severity", row.severity);
            optional_string(&mut fields, "severity_label", row.severity_label.as_deref());
            for (index, arg) in row.args.iter().enumerate() {
                fields.push(string(&format!("arg{}_type", index + 1), &arg.type_name));
                fields.push(string(&format!("arg{}_value", index + 1), &arg.display));
            }
            event_line("message_log", &row.common, "message", fields)
        }
        DomainRow::State(row) => event_line(
            "state_history",
            &row.common,
            "state",
            vec![
                string("state_machine", &row.state_machine),
                string("state_category", &row.state_category),
                string("previous_state", &row.previous_state),
                string("new_state", &row.new_state),
            ],
        ),
        DomainRow::Batch(row) => event_line(
            "batch_history",
            &row.common,
            "batch",
            vec![
                string("batch_id", &row.batch_id),
                string("new_state", &row.new_state),
            ],
        ),
        DomainRow::Recipe(row) => event_line(
            "recipe_history",
            &row.common,
            "recipe",
            vec![
                string("action", &row.action),
                string("recipe_id", &row.recipe_id),
            ],
        ),
        DomainRow::Material(row) => event_line(
            "material_additions",
            &row.common,
            "material",
            vec![
                string("batch_id", &row.batch_id),
                string("material_id", &row.material_id),
                float("quantity", row.quantity),
                string("exact_quantity", &row.exact_quantity),
            ],
        ),
        DomainRow::Operator(row) => event_line(
            "operator_activity",
            &row.common,
            "operator",
            vec![string("action", &row.action)],
        ),
        DomainRow::Audit(row) => event_line(
            "audit_log",
            &row.common,
            "audit",
            vec![
                string("action", &row.action),
                string("target", &row.target),
                string("actor", &row.actor),
                string("reason", &row.reason),
                string("value_type", &row.value_type),
                string("previous_value", &row.previous_value),
                string("current_value", &row.current_value),
            ],
        ),
        DomainRow::Signature(row) => event_line(
            "electronic_signatures",
            &row.common,
            "signature",
            vec![
                string("action_id", &row.action_id),
                string("actor", &row.actor),
                string("meaning", &row.meaning),
                uint("signed_source_id", row.signed_source_id),
                unsigned_text("signed_sequence", &row.signed_sequence),
            ],
        ),
        DomainRow::System(row) => {
            let mut fields = vec![string("event_name", &row.event_name)];
            optional_uint(&mut fields, "interval_ms", row.interval_ms);
            optional_unsigned_text(&mut fields, "sequence_base", row.sequence_base.as_deref());
            optional_unsigned_text(&mut fields, "dropped_count", row.dropped_count.as_deref());
            optional_unsigned_text(&mut fields, "first_sequence", row.first_sequence.as_deref());
            optional_unsigned_text(&mut fields, "last_sequence", row.last_sequence.as_deref());
            optional_unsigned_text(
                &mut fields,
                "registered_source_id",
                row.registered_source_id.as_deref(),
            );
            optional_string(
                &mut fields,
                "previous_definition_hash",
                row.previous_definition_hash.as_deref(),
            );
            optional_string(
                &mut fields,
                "new_definition_hash",
                row.new_definition_hash.as_deref(),
            );
            optional_unsigned_text(
                &mut fields,
                "changed_epoch_id",
                row.changed_epoch_id.as_deref(),
            );
            optional_string(&mut fields, "clock_quality", row.clock_quality.as_deref());
            optional_unsigned_text(&mut fields, "produced_count", row.produced_count.as_deref());
            optional_bool(&mut fields, "cold_start", row.cold_start);
            event_line("system_events", &row.common, "system", fields)
        }
        DomainRow::Loss(row) => custom_line(
            "data_loss",
            &row.record_id,
            &row.run_id,
            row.source_id,
            &row.received_time_ns,
            vec![
                unsigned_text("first_sequence", &row.first_sequence),
                unsigned_text("last_sequence", &row.last_sequence),
                unsigned_text("lost_count", &row.lost_count),
                string("basis", row.basis),
            ],
        ),
        DomainRow::Unresolved(row) => custom_line(
            "unresolved_records",
            &row.record_id,
            &row.run_id,
            row.source_id,
            row.event_time_ns
                .as_deref()
                .unwrap_or(&row.received_time_ns),
            vec![
                uint("event_type_id", row.event_type_id),
                string("reason", &row.reason),
            ],
        ),
    }
}

fn event_line(
    measurement: &str,
    row: &EventLogRow,
    part: &str,
    mut fields: Vec<String>,
) -> Result<String, PersistenceError> {
    fields.push(unsigned_text("received_time_ns", &row.received_time_ns));
    optional_string(&mut fields, "source", row.source.as_deref());
    fields.push(string("source_path", &row.source_path));
    fields.push(string("source_hierarchy", &row.source_hierarchy));
    fields.push(uint("buffer_id", row.buffer_id));
    fields.push(unsigned_text("epoch_id", &row.epoch_id));
    fields.push(unsigned_text("sequence", &row.sequence));
    fields.push(string("definition_hash", &row.definition_hash));
    fields.push(boolean("time_unsynced", row.time_unsynced));
    fields.push(boolean("synthetic_record", row.synthetic_record));
    fields.push(boolean("partial_payload", row.partial_payload));
    custom_line(
        measurement,
        &row.record_id,
        &row.run_id,
        row.source_id,
        row.event_time_ns
            .as_deref()
            .unwrap_or(&row.received_time_ns),
        fields,
    )
    .map(|line| line.replacen("part_id=projection", &format!("part_id={part}"), 1))
}

fn custom_line(
    measurement: &str,
    record_id: &str,
    run_id: &str,
    source_id: u32,
    timestamp: &str,
    fields: Vec<String>,
) -> Result<String, PersistenceError> {
    let timestamp = timestamp.parse::<i64>().map_err(|error| {
        PersistenceError::Commit(format!("InfluxDB 3 point timestamp is invalid: {error}"))
    })?;
    Ok(format!("{measurement},record_id={},run_id={},source_id={source_id},part_id=projection {} {timestamp}", super::influxdb3::escape_tag(record_id), super::influxdb3::escape_tag(run_id), fields.join(",")))
}

fn string(name: &str, value: &str) -> String {
    format!("{name}=\"{}\"", super::influxdb3::escape_field(value))
}
fn boolean(name: &str, value: bool) -> String {
    format!("{name}={value}")
}
fn uint(name: &str, value: impl std::fmt::Display) -> String {
    format!("{name}={value}u")
}
fn unsigned_text(name: &str, value: &str) -> String {
    format!("{name}={value}u")
}
fn float(name: &str, value: f64) -> String {
    format!("{name}={value}")
}
fn signed(name: &str, value: i64) -> String {
    format!("{name}={value}i")
}
fn optional_string(fields: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push(string(name, value));
    }
}
fn optional_bool(fields: &mut Vec<String>, name: &str, value: Option<bool>) {
    if let Some(value) = value {
        fields.push(boolean(name, value));
    }
}
fn optional_uint(fields: &mut Vec<String>, name: &str, value: Option<impl std::fmt::Display>) {
    if let Some(value) = value {
        fields.push(uint(name, value));
    }
}
fn optional_signed(fields: &mut Vec<String>, name: &str, value: Option<i64>) {
    if let Some(value) = value {
        fields.push(signed(name, value));
    }
}
fn optional_unsigned_text(fields: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push(unsigned_text(name, value));
    }
}
fn optional_float(fields: &mut Vec<String>, name: &str, value: Option<f64>) {
    if let Some(value) = value {
        fields.push(float(name, value));
    }
}
