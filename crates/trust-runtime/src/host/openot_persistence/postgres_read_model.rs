use postgres::{types::ToSql, Statement, Transaction};

use super::projection::{EventLogRow, LoggedValueRow, ProjectedDocument};
use super::projection_domains::DomainRow;
use super::PersistenceError;

#[derive(Default)]
pub(super) struct StatementCache(std::collections::HashMap<&'static str, Statement>);

impl StatementCache {
    fn get(
        &mut self,
        transaction: &mut Transaction<'_>,
        key: &'static str,
        sql: &str,
    ) -> Result<Statement, PersistenceError> {
        if let Some(statement) = self.0.get(key) {
            return Ok(statement.clone());
        }
        let statement = transaction
            .prepare(sql)
            .map_err(error("prepare domain projection"))?;
        self.0.insert(key, statement.clone());
        Ok(statement)
    }
}

const COMMON_COLUMNS: &str = "record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload";
const COMMON_SELECT: &str = "e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload";
const COMMON_DDL: &str = "record_id TEXT NOT NULL,event_time TIMESTAMPTZ,event_time_ns NUMERIC(20),received_time TIMESTAMPTZ NOT NULL,received_time_ns NUMERIC(20) NOT NULL,source TEXT,source_id BIGINT NOT NULL,source_path TEXT NOT NULL,source_hierarchy TEXT NOT NULL,buffer_id BIGINT NOT NULL,run_id NUMERIC(20) NOT NULL,epoch_id NUMERIC(20) NOT NULL,sequence NUMERIC(20) NOT NULL,definition_hash TEXT NOT NULL,time_unsynced BOOLEAN NOT NULL,synthetic_record BOOLEAN NOT NULL,partial_payload BOOLEAN NOT NULL";

pub(super) fn create_domain_schema(
    transaction: &mut Transaction<'_>,
    schema: &str,
) -> Result<(), PersistenceError> {
    let domains = [
        ("message_log", "message_template TEXT NOT NULL,severity INTEGER,severity_label TEXT,arg1_type TEXT,arg1_value TEXT,arg2_type TEXT,arg2_value TEXT,arg3_type TEXT,arg3_value TEXT,arg4_type TEXT,arg4_value TEXT"),
        ("state_history", "state_machine TEXT NOT NULL,state_category TEXT NOT NULL,previous_state TEXT NOT NULL,previous_state_label TEXT,new_state TEXT NOT NULL,new_state_label TEXT"),
        ("batch_history", "batch_id TEXT NOT NULL,recipe_id TEXT,previous_state TEXT,new_state TEXT NOT NULL,new_state_label TEXT"),
        ("recipe_history", "action TEXT NOT NULL,recipe_id TEXT NOT NULL,recipe_version TEXT,batch_id TEXT,actor TEXT,authorization_result TEXT"),
        ("material_additions", "batch_id TEXT NOT NULL,material_id TEXT NOT NULL,quantity DOUBLE PRECISION NOT NULL,exact_quantity TEXT NOT NULL,unit TEXT"),
        ("operator_activity", "action TEXT NOT NULL,action_id TEXT,actor TEXT,workstation TEXT,role TEXT,authorization_result TEXT,reason TEXT,context_references TEXT"),
        ("audit_log", "action TEXT NOT NULL,target TEXT NOT NULL,actor TEXT NOT NULL,reason TEXT NOT NULL,authorization_result TEXT,value_type TEXT NOT NULL,previous_value TEXT NOT NULL,current_value TEXT NOT NULL,workstation TEXT"),
        ("electronic_signatures", "action_id TEXT NOT NULL,actor TEXT NOT NULL,meaning TEXT NOT NULL,authorization_result TEXT,signed_source_id BIGINT NOT NULL,signed_sequence NUMERIC(20) NOT NULL"),
        ("system_events", "system_event_name TEXT NOT NULL,interval_ms NUMERIC(20),sequence_base NUMERIC(20),dropped_count NUMERIC(20),first_sequence NUMERIC(20),last_sequence NUMERIC(20),registered_source_id NUMERIC(20),previous_definition_hash TEXT,new_definition_hash TEXT,changed_epoch_id NUMERIC(20),clock_quality TEXT,produced_count NUMERIC(20),cold_start BOOLEAN"),
    ];
    for (table, columns) in domains {
        let common = COMMON_DDL;
        transaction
            .batch_execute(&format!(
                "CREATE TABLE IF NOT EXISTS \"{schema}\".{table} ({common},
                 {columns}, PRIMARY KEY(record_id),
                 FOREIGN KEY(record_id) REFERENCES \"{schema}\".logging_records(identity_key));"
            ))
            .map_err(error("create PostgreSQL domain table"))?;
    }
    let common = COMMON_DDL;
    transaction
        .batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{schema}\".data_loss ({common},
             first_sequence NUMERIC(20) NOT NULL,last_sequence NUMERIC(20) NOT NULL,
             lost_count NUMERIC(20) NOT NULL,basis TEXT NOT NULL,PRIMARY KEY(record_id),
             FOREIGN KEY(record_id) REFERENCES \"{schema}\".logging_records(identity_key));
             CREATE TABLE IF NOT EXISTS \"{schema}\".unresolved_records ({common},
             unresolved_event_type_id BIGINT NOT NULL,reason TEXT NOT NULL,diagnostic_summary TEXT,
             PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES \"{schema}\".logging_records(identity_key));"
        ))
        .map_err(error("create PostgreSQL loss and unresolved tables"))
}

pub(super) fn insert_projection(
    transaction: &mut Transaction<'_>,
    schema: &str,
    values: Vec<LoggedValueRow>,
    domains: Vec<DomainRow>,
    statements: &mut StatementCache,
) -> Result<(), PersistenceError> {
    for value in values {
        insert_value(transaction, schema, value)?;
    }
    for domain in domains {
        match domain {
            DomainRow::Alarm(alarm) => {
                let common = alarm.common;
                let sql = format!(
                    "INSERT INTO \"{schema}\".alarm_history (
                     record_id,event_time,event_time_ns,received_time,received_time_ns,
                     source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,
                     sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,
                     condition,condition_class,lifecycle_action)
                     VALUES($1,$2::text::timestamptz,$3::text::numeric,$4::text::timestamptz,$5::text::numeric,
                     $6,$7,$8,$9,$10,$11::text::numeric,$12::text::numeric,$13::text::numeric,$14,$15,$16,$17,
                     $18,$19,$20)"
                );
                let statement = statements.get(transaction, "alarm_history", &sql)?;
                transaction
                    .execute(
                        &statement,
                    &[
                        &common.record_id,
                        &common.event_time,
                        &common.event_time_ns,
                        &common.received_time,
                        &common.received_time_ns,
                        &common.source,
                        &i64::from(common.source_id),
                        &common.source_path,
                        &common.source_hierarchy,
                        &i64::from(common.buffer_id),
                        &common.run_id,
                        &common.epoch_id,
                        &common.sequence,
                        &common.definition_hash,
                        &common.time_unsynced,
                        &common.synthetic_record,
                        &common.partial_payload,
                        &alarm.condition,
                        &alarm.condition_class,
                        &alarm.lifecycle_action,
                    ],
                )
                .map_err(error("insert alarm projection"))?;
            }
            DomainRow::Message(row) => {
                let args = message_args(&row.args);
                insert_event_domain(transaction, statements, schema, "message_log", "message_template,severity,severity_label,arg1_type,arg1_value,arg2_type,arg2_value,arg3_type,arg3_value,arg4_type,arg4_value", "$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12", &[&row.common.record_id,&row.message_template,&row.severity.map(i32::from),&row.severity_label,&args[0].0,&args[0].1,&args[1].0,&args[1].1,&args[2].0,&args[2].1,&args[3].0,&args[3].1])?;
            }
            DomainRow::State(row) => insert_event_domain(transaction, statements, schema, "state_history", "state_machine,state_category,previous_state,previous_state_label,new_state,new_state_label", "$2,$3,$4,$5,$6,$7", &[&row.common.record_id,&row.state_machine,&row.state_category,&row.previous_state,&row.previous_state_label,&row.new_state,&row.new_state_label])?,
            DomainRow::Batch(row) => insert_event_domain(transaction, statements, schema, "batch_history", "batch_id,recipe_id,previous_state,new_state,new_state_label", "$2,$3,$4,$5,$6", &[&row.common.record_id,&row.batch_id,&row.recipe_id,&row.previous_state,&row.new_state,&row.new_state_label])?,
            DomainRow::Recipe(row) => insert_event_domain(transaction, statements, schema, "recipe_history", "action,recipe_id,recipe_version,batch_id,actor,authorization_result", "$2,$3,$4,$5,$6,$7", &[&row.common.record_id,&row.action,&row.recipe_id,&row.recipe_version,&row.batch_id,&row.actor,&row.authorization_result])?,
            DomainRow::Material(row) => insert_event_domain(transaction, statements, schema, "material_additions", "batch_id,material_id,quantity,exact_quantity,unit", "$2,$3,$4,$5,$6", &[&row.common.record_id,&row.batch_id,&row.material_id,&row.quantity,&row.exact_quantity,&row.unit])?,
            DomainRow::Operator(row) => insert_event_domain(transaction, statements, schema, "operator_activity", "action,action_id,actor,workstation,role,authorization_result,reason,context_references", "$2,$3,$4,$5,$6,$7,$8,$9", &[&row.common.record_id,&row.action,&row.action_id,&row.actor,&row.workstation,&row.role,&row.authorization_result,&row.reason,&row.context_references])?,
            DomainRow::Audit(row) => insert_event_domain(transaction, statements, schema, "audit_log", "action,target,actor,reason,authorization_result,value_type,previous_value,current_value,workstation", "$2,$3,$4,$5,$6,$7,$8,$9,$10", &[&row.common.record_id,&row.action,&row.target,&row.actor,&row.reason,&row.authorization_result,&row.value_type,&row.previous_value,&row.current_value,&row.workstation])?,
            DomainRow::Signature(row) => insert_event_domain(transaction, statements, schema, "electronic_signatures", "action_id,actor,meaning,authorization_result,signed_source_id,signed_sequence", "$2,$3,$4,$5,$6,$7::text::numeric", &[&row.common.record_id,&row.action_id,&row.actor,&row.meaning,&row.authorization_result,&i64::from(row.signed_source_id),&row.signed_sequence])?,
            DomainRow::System(row) => insert_event_domain(transaction, statements, schema, "system_events", "system_event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start", "$2,$3::text::numeric,$4::text::numeric,$5::text::numeric,$6::text::numeric,$7::text::numeric,$8::text::numeric,$9,$10,$11::text::numeric,$12,$13::text::numeric,$14", &[&row.common.record_id,&row.event_name,&row.interval_ms.map(|value| value.to_string()),&row.sequence_base,&row.dropped_count,&row.first_sequence,&row.last_sequence,&row.registered_source_id,&row.previous_definition_hash,&row.new_definition_hash,&row.changed_epoch_id,&row.clock_quality,&row.produced_count,&row.cold_start])?,
            DomainRow::Loss(row) => insert_loss(transaction, schema, row)?,
            DomainRow::Unresolved(row) => insert_unresolved(transaction, schema, row)?,
        }
    }
    Ok(())
}

pub(super) fn insert_event_batch(
    transaction: &mut Transaction<'_>,
    schema: &str,
    events: &[&EventLogRow],
) -> Result<(), PersistenceError> {
    if events.is_empty() {
        return Ok(());
    }
    let record_ids = events
        .iter()
        .map(|row| row.record_id.as_str())
        .collect::<Vec<_>>();
    let event_times = events
        .iter()
        .map(|row| row.event_time.as_deref())
        .collect::<Vec<_>>();
    let event_time_ns = events
        .iter()
        .map(|row| row.event_time_ns.as_deref())
        .collect::<Vec<_>>();
    let received_times = events
        .iter()
        .map(|row| row.received_time.as_str())
        .collect::<Vec<_>>();
    let received_time_ns = events
        .iter()
        .map(|row| row.received_time_ns.as_str())
        .collect::<Vec<_>>();
    let sources = events
        .iter()
        .map(|row| row.source.as_deref())
        .collect::<Vec<_>>();
    let source_ids = events
        .iter()
        .map(|row| i64::from(row.source_id))
        .collect::<Vec<_>>();
    let source_paths = events
        .iter()
        .map(|row| row.source_path.as_str())
        .collect::<Vec<_>>();
    let source_hierarchies = events
        .iter()
        .map(|row| row.source_hierarchy.as_str())
        .collect::<Vec<_>>();
    let buffer_ids = events
        .iter()
        .map(|row| i64::from(row.buffer_id))
        .collect::<Vec<_>>();
    let run_ids = events
        .iter()
        .map(|row| row.run_id.as_str())
        .collect::<Vec<_>>();
    let epoch_ids = events
        .iter()
        .map(|row| row.epoch_id.as_str())
        .collect::<Vec<_>>();
    let sequences = events
        .iter()
        .map(|row| row.sequence.as_str())
        .collect::<Vec<_>>();
    let definition_hashes = events
        .iter()
        .map(|row| row.definition_hash.as_str())
        .collect::<Vec<_>>();
    let time_unsynced = events
        .iter()
        .map(|row| row.time_unsynced)
        .collect::<Vec<_>>();
    let synthetic_records = events
        .iter()
        .map(|row| row.synthetic_record)
        .collect::<Vec<_>>();
    let partial_payloads = events
        .iter()
        .map(|row| row.partial_payload)
        .collect::<Vec<_>>();
    let event_type_ids = events
        .iter()
        .map(|row| i64::from(row.event_type_id))
        .collect::<Vec<_>>();
    let event_names = events
        .iter()
        .map(|row| row.event_name.as_str())
        .collect::<Vec<_>>();
    let has_unclassified_fields = events
        .iter()
        .map(|row| row.has_unclassified_fields)
        .collect::<Vec<_>>();
    transaction.execute(
        &format!("INSERT INTO \"{schema}\".event_log ({COMMON_COLUMNS},event_type_id,event_name,has_unclassified_fields) SELECT record_id,event_time::timestamptz,event_time_ns::numeric,received_time::timestamptz,received_time_ns::numeric,source,source_id,source_path,source_hierarchy,buffer_id,run_id::numeric,epoch_id::numeric,sequence::numeric,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields FROM UNNEST($1::text[],$2::text[],$3::text[],$4::text[],$5::text[],$6::text[],$7::bigint[],$8::text[],$9::text[],$10::bigint[],$11::text[],$12::text[],$13::text[],$14::text[],$15::boolean[],$16::boolean[],$17::boolean[],$18::bigint[],$19::text[],$20::boolean[]) AS rows(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields)"),
        &[&record_ids,&event_times,&event_time_ns,&received_times,&received_time_ns,&sources,&source_ids,&source_paths,&source_hierarchies,&buffer_ids,&run_ids,&epoch_ids,&sequences,&definition_hashes,&time_unsynced,&synthetic_records,&partial_payloads,&event_type_ids,&event_names,&has_unclassified_fields],
    ).map(|_| ()).map_err(error("insert event projection batch"))
}

pub(super) fn insert_high_volume_domain_batches(
    transaction: &mut Transaction<'_>,
    schema: &str,
    documents: &[ProjectedDocument],
) -> Result<(), PersistenceError> {
    let alarms = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::Alarm(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !alarms.is_empty() {
        let record_ids = alarms
            .iter()
            .map(|row| row.common.record_id.as_str())
            .collect::<Vec<_>>();
        let conditions = alarms
            .iter()
            .map(|row| row.condition.as_str())
            .collect::<Vec<_>>();
        let classes = alarms
            .iter()
            .map(|row| row.condition_class.as_deref())
            .collect::<Vec<_>>();
        let actions = alarms
            .iter()
            .map(|row| row.lifecycle_action.as_str())
            .collect::<Vec<_>>();
        transaction.execute(
            &format!("INSERT INTO \"{schema}\".alarm_history ({COMMON_COLUMNS},condition,condition_class,lifecycle_action) SELECT {COMMON_SELECT},rows.condition,rows.condition_class,rows.lifecycle_action FROM \"{schema}\".event_log e JOIN UNNEST($1::text[],$2::text[],$3::text[],$4::text[]) AS rows(record_id,condition,condition_class,lifecycle_action) ON e.record_id=rows.record_id"),
            &[&record_ids, &conditions, &classes, &actions],
        ).map_err(error("insert alarm projection batch"))?;
    }

    let systems = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::System(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !systems.is_empty() {
        let record_ids = systems
            .iter()
            .map(|row| row.common.record_id.as_str())
            .collect::<Vec<_>>();
        let names = systems
            .iter()
            .map(|row| row.event_name.as_str())
            .collect::<Vec<_>>();
        let intervals = systems
            .iter()
            .map(|row| row.interval_ms.map(|value| value.to_string()))
            .collect::<Vec<_>>();
        let sequence_bases = systems
            .iter()
            .map(|row| row.sequence_base.as_deref())
            .collect::<Vec<_>>();
        let dropped_counts = systems
            .iter()
            .map(|row| row.dropped_count.as_deref())
            .collect::<Vec<_>>();
        let first_sequences = systems
            .iter()
            .map(|row| row.first_sequence.as_deref())
            .collect::<Vec<_>>();
        let last_sequences = systems
            .iter()
            .map(|row| row.last_sequence.as_deref())
            .collect::<Vec<_>>();
        let registered_sources = systems
            .iter()
            .map(|row| row.registered_source_id.as_deref())
            .collect::<Vec<_>>();
        let previous_hashes = systems
            .iter()
            .map(|row| row.previous_definition_hash.as_deref())
            .collect::<Vec<_>>();
        let new_hashes = systems
            .iter()
            .map(|row| row.new_definition_hash.as_deref())
            .collect::<Vec<_>>();
        let changed_epochs = systems
            .iter()
            .map(|row| row.changed_epoch_id.as_deref())
            .collect::<Vec<_>>();
        let clock_qualities = systems
            .iter()
            .map(|row| row.clock_quality.as_deref())
            .collect::<Vec<_>>();
        let produced_counts = systems
            .iter()
            .map(|row| row.produced_count.as_deref())
            .collect::<Vec<_>>();
        let cold_starts = systems.iter().map(|row| row.cold_start).collect::<Vec<_>>();
        transaction.execute(
            &format!("INSERT INTO \"{schema}\".system_events ({COMMON_COLUMNS},system_event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start) SELECT {COMMON_SELECT},rows.system_event_name,rows.interval_ms::numeric,rows.sequence_base::numeric,rows.dropped_count::numeric,rows.first_sequence::numeric,rows.last_sequence::numeric,rows.registered_source_id::numeric,rows.previous_definition_hash,rows.new_definition_hash,rows.changed_epoch_id::numeric,rows.clock_quality,rows.produced_count::numeric,rows.cold_start FROM \"{schema}\".event_log e JOIN UNNEST($1::text[],$2::text[],$3::text[],$4::text[],$5::text[],$6::text[],$7::text[],$8::text[],$9::text[],$10::text[],$11::text[],$12::text[],$13::text[],$14::boolean[]) AS rows(record_id,system_event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start) ON e.record_id=rows.record_id"),
            &[&record_ids,&names,&intervals,&sequence_bases,&dropped_counts,&first_sequences,&last_sequences,&registered_sources,&previous_hashes,&new_hashes,&changed_epochs,&clock_qualities,&produced_counts,&cold_starts],
        ).map_err(error("insert system projection batch"))?;
    }
    let operators = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::Operator(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !operators.is_empty() {
        let record_ids = operators
            .iter()
            .map(|row| row.common.record_id.as_str())
            .collect::<Vec<_>>();
        let actions = operators
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>();
        let action_ids = operators
            .iter()
            .map(|row| row.action_id.as_deref())
            .collect::<Vec<_>>();
        let actors = operators
            .iter()
            .map(|row| row.actor.as_deref())
            .collect::<Vec<_>>();
        let workstations = operators
            .iter()
            .map(|row| row.workstation.as_deref())
            .collect::<Vec<_>>();
        let roles = operators
            .iter()
            .map(|row| row.role.as_deref())
            .collect::<Vec<_>>();
        let authorization_results = operators
            .iter()
            .map(|row| row.authorization_result.as_deref())
            .collect::<Vec<_>>();
        let reasons = operators
            .iter()
            .map(|row| row.reason.as_deref())
            .collect::<Vec<_>>();
        let contexts = operators
            .iter()
            .map(|row| row.context_references.as_deref())
            .collect::<Vec<_>>();
        transaction.execute(
            &format!("INSERT INTO \"{schema}\".operator_activity ({COMMON_COLUMNS},action,action_id,actor,workstation,role,authorization_result,reason,context_references) SELECT {COMMON_SELECT},rows.action,rows.action_id,rows.actor,rows.workstation,rows.role,rows.authorization_result,rows.reason,rows.context_references FROM \"{schema}\".event_log e JOIN UNNEST($1::text[],$2::text[],$3::text[],$4::text[],$5::text[],$6::text[],$7::text[],$8::text[],$9::text[]) AS rows(record_id,action,action_id,actor,workstation,role,authorization_result,reason,context_references) ON e.record_id=rows.record_id"),
            &[&record_ids,&actions,&action_ids,&actors,&workstations,&roles,&authorization_results,&reasons,&contexts],
        ).map_err(error("insert operator projection batch"))?;
    }
    let recipes = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::Recipe(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !recipes.is_empty() {
        let record_ids = recipes
            .iter()
            .map(|row| row.common.record_id.as_str())
            .collect::<Vec<_>>();
        let actions = recipes
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>();
        let recipe_ids = recipes
            .iter()
            .map(|row| row.recipe_id.as_str())
            .collect::<Vec<_>>();
        let versions = recipes
            .iter()
            .map(|row| row.recipe_version.as_deref())
            .collect::<Vec<_>>();
        let batch_ids = recipes
            .iter()
            .map(|row| row.batch_id.as_deref())
            .collect::<Vec<_>>();
        let actors = recipes
            .iter()
            .map(|row| row.actor.as_deref())
            .collect::<Vec<_>>();
        let authorization_results = recipes
            .iter()
            .map(|row| row.authorization_result.as_deref())
            .collect::<Vec<_>>();
        transaction.execute(
            &format!("INSERT INTO \"{schema}\".recipe_history ({COMMON_COLUMNS},action,recipe_id,recipe_version,batch_id,actor,authorization_result) SELECT {COMMON_SELECT},rows.action,rows.recipe_id,rows.recipe_version,rows.batch_id,rows.actor,rows.authorization_result FROM \"{schema}\".event_log e JOIN UNNEST($1::text[],$2::text[],$3::text[],$4::text[],$5::text[],$6::text[],$7::text[]) AS rows(record_id,action,recipe_id,recipe_version,batch_id,actor,authorization_result) ON e.record_id=rows.record_id"),
            &[&record_ids,&actions,&recipe_ids,&versions,&batch_ids,&actors,&authorization_results],
        ).map_err(error("insert recipe projection batch"))?;
    }
    Ok(())
}

fn insert_event_domain(
    transaction: &mut Transaction<'_>,
    statements: &mut StatementCache,
    schema: &str,
    table: &str,
    columns: &str,
    placeholders: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<(), PersistenceError> {
    let sql = format!("INSERT INTO \"{schema}\".{table} ({COMMON_COLUMNS},{columns}) SELECT {COMMON_SELECT},{placeholders} FROM \"{schema}\".event_log e WHERE e.record_id=$1");
    let key = match table {
        "message_log" => "message_log",
        "state_history" => "state_history",
        "batch_history" => "batch_history",
        "recipe_history" => "recipe_history",
        "material_additions" => "material_additions",
        "operator_activity" => "operator_activity",
        "audit_log" => "audit_log",
        "electronic_signatures" => "electronic_signatures",
        "system_events" => "system_events",
        _ => {
            return Err(PersistenceError::Commit(
                "unknown PostgreSQL domain table".into(),
            ))
        }
    };
    let statement = statements.get(transaction, key, &sql)?;
    transaction
        .execute(&statement, params)
        .map(|_| ())
        .map_err(error("insert PostgreSQL domain projection"))
}

fn message_args(
    args: &[super::projection_domains::TypedDisplay],
) -> [(Option<&str>, Option<&str>); 4] {
    std::array::from_fn(|index| {
        args.get(index).map_or((None, None), |arg| {
            (Some(arg.type_name.as_str()), Some(arg.display.as_str()))
        })
    })
}

fn insert_loss(
    transaction: &mut Transaction<'_>,
    schema: &str,
    row: super::projection_domains::LossRow,
) -> Result<(), PersistenceError> {
    transaction.execute(
        &format!("INSERT INTO \"{schema}\".data_loss ({COMMON_COLUMNS},first_sequence,last_sequence,lost_count,basis) VALUES($1,NULL,NULL,$2::text::timestamptz,$3::text::numeric,$4,$5,$6,$7,$8,$9::text::numeric,$10::text::numeric,$11::text::numeric,$12,$13,$14,$15,$16::text::numeric,$17::text::numeric,$18::text::numeric,$19)"),
        &[&row.record_id,&row.received_time,&row.received_time_ns,&row.source,&i64::from(row.source_id),&row.source_path,&row.source_hierarchy,&i64::from(row.buffer_id),&row.run_id,&row.epoch_id,&row.first_sequence,&row.definition_hash,&row.time_unsynced,&row.synthetic_record,&row.partial_payload,&row.first_sequence,&row.last_sequence,&row.lost_count,&row.basis],
    ).map(|_| ()).map_err(error("insert PostgreSQL data loss projection"))
}

fn insert_unresolved(
    transaction: &mut Transaction<'_>,
    schema: &str,
    row: super::projection_domains::UnresolvedRow,
) -> Result<(), PersistenceError> {
    transaction.execute(
        &format!("INSERT INTO \"{schema}\".unresolved_records ({COMMON_COLUMNS},unresolved_event_type_id,reason,diagnostic_summary) VALUES($1,$2::text::timestamptz,$3::text::numeric,$4::text::timestamptz,$5::text::numeric,$6,$7,$8,$9,$10,$11::text::numeric,$12::text::numeric,$13::text::numeric,$14,$15,$16,$17,$18,$19,$20)"),
        &[&row.record_id,&row.event_time,&row.event_time_ns,&row.received_time,&row.received_time_ns,&row.source,&i64::from(row.source_id),&row.source_path,&row.source_hierarchy,&i64::from(row.buffer_id),&row.run_id,&row.epoch_id,&row.sequence,&row.definition_hash,&row.time_unsynced,&row.synthetic_record,&row.partial_payload,&i64::from(row.event_type_id),&row.reason,&row.diagnostic_summary],
    ).map(|_| ()).map_err(error("insert PostgreSQL unresolved projection"))
}

fn insert_value(
    transaction: &mut Transaction<'_>,
    schema: &str,
    value: LoggedValueRow,
) -> Result<(), PersistenceError> {
    let common = value.common;
    transaction
        .execute(
            &format!(
                "INSERT INTO \"{schema}\".logged_values (
                 record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,
                 source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,
                 time_unsynced,synthetic_record,partial_payload,value_id,value_name,value_type,unit,
                 quality,semantic_role,boolean_value,signed_value,unsigned_value,number_value,
                 text_value,exact_value,previous_boolean_value,previous_signed_value,
                 previous_unsigned_value,previous_number_value,previous_text_value,
                 previous_exact_value,is_audited,actor,reason,authorization_result)
                 VALUES($1,$2::text::timestamptz,$3::text::numeric,$4::text::timestamptz,$5::text::numeric,$6,$7,$8,$9,$10,
                 $11::text::numeric,$12::text::numeric,$13::text::numeric,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,
                 $25,$26::text::numeric,$27,$28,$29,$30,$31,$32::text::numeric,$33,$34,$35,$36,$37,$38,$39)"
            ),
            &[
                &common.record_id,
                &common.event_time,
                &common.event_time_ns,
                &common.received_time,
                &common.received_time_ns,
                &common.source,
                &i64::from(common.source_id),
                &common.source_path,
                &common.source_hierarchy,
                &i64::from(common.buffer_id),
                &common.run_id,
                &common.epoch_id,
                &common.sequence,
                &common.definition_hash,
                &common.time_unsynced,
                &common.synthetic_record,
                &common.partial_payload,
                &i64::from(value.value_id),
                &value.value_name,
                &value.value_type,
                &value.unit,
                &value.quality.map(i32::from),
                &i32::from(value.semantic_role),
                &value.boolean_value,
                &value.signed_value,
                &value.unsigned_value,
                &value.number_value,
                &value.text_value,
                &value.exact_value,
                &value.previous_boolean_value,
                &value.previous_signed_value,
                &value.previous_unsigned_value,
                &value.previous_number_value,
                &value.previous_text_value,
                &value.previous_exact_value,
                &value.is_audited,
                &value.actor,
                &value.reason,
                &value.authorization_result,
            ],
        )
        .map(|_| ())
        .map_err(error("insert logged value projection"))
}

fn error(context: &'static str) -> impl FnOnce(postgres::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("PostgreSQL {context}: {error}"))
}
