use rusqlite::{params, Transaction};

use super::projection::EventLogRow;
use super::projection_domains::{DomainRow, TypedDisplay};
use super::PersistenceError;

pub(super) fn create_domain_schema(transaction: &Transaction<'_>) -> Result<(), PersistenceError> {
    transaction.execute_batch(
        "CREATE TABLE logging_alarm_history(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),condition TEXT NOT NULL,condition_class TEXT,lifecycle_action TEXT NOT NULL,correlation_id TEXT,severity INTEGER,severity_label TEXT,cause TEXT,actor TEXT,reason TEXT,comment TEXT,shelve_seconds TEXT,previous_priority INTEGER,new_priority INTEGER);
         CREATE TABLE logging_message_log(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),message_template TEXT NOT NULL,severity INTEGER,severity_label TEXT,arg1_type TEXT,arg1_value TEXT,arg2_type TEXT,arg2_value TEXT,arg3_type TEXT,arg3_value TEXT,arg4_type TEXT,arg4_value TEXT);
         CREATE TABLE logging_state_history(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),state_machine TEXT NOT NULL,state_category TEXT NOT NULL,previous_state TEXT NOT NULL,previous_state_label TEXT,new_state TEXT NOT NULL,new_state_label TEXT);
         CREATE TABLE logging_batch_history(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),batch_id TEXT NOT NULL,recipe_id TEXT,previous_state TEXT,new_state TEXT NOT NULL,new_state_label TEXT);
         CREATE TABLE logging_recipe_history(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),action TEXT NOT NULL,recipe_id TEXT NOT NULL,recipe_version TEXT,batch_id TEXT,actor TEXT,authorization_result TEXT);
         CREATE TABLE logging_material_additions(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),batch_id TEXT NOT NULL,material_id TEXT NOT NULL,quantity REAL NOT NULL,exact_quantity TEXT NOT NULL,unit TEXT);
         CREATE TABLE logging_operator_activity(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),action TEXT NOT NULL,action_id TEXT,actor TEXT,workstation TEXT,role TEXT,authorization_result TEXT,reason TEXT,context_references TEXT);
         CREATE TABLE logging_audit_log(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),action TEXT NOT NULL,target TEXT NOT NULL,actor TEXT NOT NULL,reason TEXT NOT NULL,authorization_result TEXT,value_type TEXT NOT NULL,previous_value TEXT NOT NULL,current_value TEXT NOT NULL,workstation TEXT);
         CREATE TABLE logging_electronic_signatures(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),action_id TEXT NOT NULL,actor TEXT NOT NULL,meaning TEXT NOT NULL,authorization_result TEXT,signed_source_id INTEGER NOT NULL,signed_sequence TEXT NOT NULL);
         CREATE TABLE logging_system_events(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),event_name TEXT NOT NULL,interval_ms TEXT,sequence_base TEXT,dropped_count TEXT,first_sequence TEXT,last_sequence TEXT,registered_source_id TEXT,previous_definition_hash TEXT,new_definition_hash TEXT,changed_epoch_id TEXT,clock_quality TEXT,produced_count TEXT,cold_start INTEGER CHECK(cold_start IS NULL OR cold_start IN(0,1)));
         CREATE TABLE logging_data_loss(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),received_time TEXT NOT NULL,received_time_ns TEXT NOT NULL,source TEXT,source_id INTEGER NOT NULL,buffer_id INTEGER NOT NULL,run_id TEXT NOT NULL,epoch_id TEXT NOT NULL,definition_hash TEXT NOT NULL,first_sequence TEXT NOT NULL,last_sequence TEXT NOT NULL,lost_count TEXT NOT NULL,basis TEXT NOT NULL);
         CREATE INDEX data_loss_source_range ON logging_data_loss(source_id,run_id,first_sequence);
         CREATE TABLE logging_unresolved_records(record_id TEXT PRIMARY KEY REFERENCES logging_records(identity_key),event_time TEXT,event_time_ns TEXT,received_time TEXT NOT NULL,received_time_ns TEXT NOT NULL,source TEXT,source_id INTEGER NOT NULL,buffer_id INTEGER NOT NULL,run_id TEXT NOT NULL,epoch_id TEXT NOT NULL,sequence TEXT NOT NULL,definition_hash TEXT NOT NULL,event_type_id INTEGER NOT NULL,reason TEXT NOT NULL,diagnostic_summary TEXT);
         CREATE INDEX unresolved_records_source_sequence ON logging_unresolved_records(source_id,run_id,sequence);
         CREATE VIEW alarm_history AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.condition,p.condition_class,p.lifecycle_action,p.correlation_id,p.severity,p.severity_label,p.cause,p.actor,p.reason,p.comment,p.shelve_seconds,p.previous_priority,p.new_priority FROM event_log e JOIN logging_alarm_history p USING(record_id);
         CREATE VIEW message_log AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_message_log p USING(record_id);
         CREATE VIEW state_history AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_state_history p USING(record_id);
         CREATE VIEW batch_history AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_batch_history p USING(record_id);
         CREATE VIEW recipe_history AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_recipe_history p USING(record_id);
         CREATE VIEW material_additions AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_material_additions p USING(record_id);
         CREATE VIEW operator_activity AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_operator_activity p USING(record_id);
         CREATE VIEW audit_log AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_audit_log p USING(record_id);
         CREATE VIEW electronic_signatures AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_electronic_signatures p USING(record_id);
         CREATE VIEW system_events AS SELECT e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload,p.* FROM event_log e JOIN logging_system_events p USING(record_id);
         CREATE VIEW data_loss AS SELECT record_id,NULL AS event_time,NULL AS event_time_ns,received_time,received_time_ns,source,source_id,'' AS source_path,'' AS source_hierarchy,buffer_id,run_id,epoch_id,first_sequence AS sequence,definition_hash,1 AS time_unsynced,1 AS synthetic_record,1 AS partial_payload,first_sequence,last_sequence,lost_count,basis FROM logging_data_loss;
         CREATE VIEW unresolved_records AS SELECT record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,'' AS source_path,'' AS source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,1 AS time_unsynced,1 AS synthetic_record,1 AS partial_payload,event_type_id,reason,diagnostic_summary FROM logging_unresolved_records;"
    ).map_err(error("create SQLite domain read model"))
}

pub(super) fn insert_domains(
    transaction: &Transaction<'_>,
    rows: Vec<DomainRow>,
) -> Result<(), PersistenceError> {
    for row in rows {
        match row {
            DomainRow::Alarm(row) => transaction.execute("INSERT INTO logging_alarm_history VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)", params![row.common.record_id,row.condition,row.condition_class,row.lifecycle_action,row.correlation_id,row.severity,row.severity_label,row.cause,row.actor,row.reason,row.comment,row.shelve_seconds.map(|value| value.to_string()),row.previous_priority,row.new_priority]),
            DomainRow::Message(row) => { let args = message_args(&row.args); transaction.execute("INSERT INTO logging_message_log VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)", params![row.common.record_id,row.message_template,row.severity,row.severity_label,args[0].0,args[0].1,args[1].0,args[1].1,args[2].0,args[2].1,args[3].0,args[3].1]) },
            DomainRow::State(row) => transaction.execute("INSERT INTO logging_state_history VALUES(?1,?2,?3,?4,?5,?6,?7)", params![row.common.record_id,row.state_machine,row.state_category,row.previous_state,row.previous_state_label,row.new_state,row.new_state_label]),
            DomainRow::Batch(row) => transaction.execute("INSERT INTO logging_batch_history VALUES(?1,?2,?3,?4,?5,?6)", params![row.common.record_id,row.batch_id,row.recipe_id,row.previous_state,row.new_state,row.new_state_label]),
            DomainRow::Recipe(row) => transaction.execute("INSERT INTO logging_recipe_history VALUES(?1,?2,?3,?4,?5,?6,?7)", params![row.common.record_id,row.action,row.recipe_id,row.recipe_version,row.batch_id,row.actor,row.authorization_result]),
            DomainRow::Material(row) => transaction.execute("INSERT INTO logging_material_additions VALUES(?1,?2,?3,?4,?5,?6)", params![row.common.record_id,row.batch_id,row.material_id,row.quantity,row.exact_quantity,row.unit]),
            DomainRow::Operator(row) => transaction.execute("INSERT INTO logging_operator_activity VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![row.common.record_id,row.action,row.action_id,row.actor,row.workstation,row.role,row.authorization_result,row.reason,row.context_references]),
            DomainRow::Audit(row) => transaction.execute("INSERT INTO logging_audit_log VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![row.common.record_id,row.action,row.target,row.actor,row.reason,row.authorization_result,row.value_type,row.previous_value,row.current_value,row.workstation]),
            DomainRow::Signature(row) => transaction.execute("INSERT INTO logging_electronic_signatures VALUES(?1,?2,?3,?4,?5,?6,?7)", params![row.common.record_id,row.action_id,row.actor,row.meaning,row.authorization_result,row.signed_source_id,row.signed_sequence]),
            DomainRow::System(row) => transaction.execute("INSERT INTO logging_system_events VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)", params![row.common.record_id,row.event_name,row.interval_ms.map(|value| value.to_string()),row.sequence_base,row.dropped_count,row.first_sequence,row.last_sequence,row.registered_source_id,row.previous_definition_hash,row.new_definition_hash,row.changed_epoch_id,row.clock_quality,row.produced_count,row.cold_start]),
            DomainRow::Loss(row) => transaction.execute("INSERT INTO logging_data_loss VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)", params![row.record_id,row.received_time,row.received_time_ns,row.source,row.source_id,row.buffer_id,row.run_id,row.epoch_id,row.definition_hash,row.first_sequence,row.last_sequence,row.lost_count,row.basis]),
            DomainRow::Unresolved(row) => transaction.execute("INSERT INTO logging_unresolved_records VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)", params![row.record_id,row.event_time,row.event_time_ns,row.received_time,row.received_time_ns,row.source,row.source_id,row.buffer_id,row.run_id,row.epoch_id,row.sequence,row.definition_hash,row.event_type_id,row.reason,row.diagnostic_summary]),
        }.map_err(error("insert SQLite domain projection"))?;
    }
    Ok(())
}

fn message_args(args: &[TypedDisplay]) -> [(Option<&str>, Option<&str>); 4] {
    std::array::from_fn(|index| {
        args.get(index).map_or((None, None), |arg| {
            (Some(arg.type_name.as_str()), Some(arg.display.as_str()))
        })
    })
}

fn error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("{context}: {error}"))
}

#[allow(dead_code)]
fn _common_contract(_common: &EventLogRow) {}
