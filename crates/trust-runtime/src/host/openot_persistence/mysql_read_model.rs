use mysql::{params, prelude::Queryable, Conn, Transaction};

use super::projection::{EventLogRow, LoggedValueRow};
use super::projection_domains::DomainRow;
use super::PersistenceError;

const COMMON_COLUMNS: &str = "record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload";
const COMMON_SELECT: &str = "e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload";
const COMMON_DDL: &str = "record_id VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,event_time DATETIME(6) NULL,event_time_ns DECIMAL(20,0) NULL,received_time DATETIME(6) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,source TEXT NULL,source_id INT UNSIGNED NOT NULL,source_path TEXT NOT NULL,source_hierarchy TEXT NOT NULL,buffer_id INT UNSIGNED NOT NULL,run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,sequence DECIMAL(20,0) NOT NULL,definition_hash TEXT NOT NULL,time_unsynced BOOLEAN NOT NULL,synthetic_record BOOLEAN NOT NULL,partial_payload BOOLEAN NOT NULL";

pub(super) fn create_domain_schema(connection: &mut Conn) -> Result<(), PersistenceError> {
    let tables = [
        ("message_log", "message_template TEXT NOT NULL,severity INT UNSIGNED NULL,severity_label TEXT NULL,arg1_type TEXT NULL,arg1_value TEXT NULL,arg2_type TEXT NULL,arg2_value TEXT NULL,arg3_type TEXT NULL,arg3_value TEXT NULL,arg4_type TEXT NULL,arg4_value TEXT NULL"),
        ("state_history", "state_machine TEXT NOT NULL,state_category TEXT NOT NULL,previous_state TEXT NOT NULL,previous_state_label TEXT NULL,new_state TEXT NOT NULL,new_state_label TEXT NULL"),
        ("batch_history", "batch_id TEXT NOT NULL,recipe_id TEXT NULL,previous_state TEXT NULL,new_state TEXT NOT NULL,new_state_label TEXT NULL"),
        ("recipe_history", "action TEXT NOT NULL,recipe_id TEXT NOT NULL,recipe_version TEXT NULL,batch_id TEXT NULL,actor TEXT NULL,authorization_result TEXT NULL"),
        ("material_additions", "batch_id TEXT NOT NULL,material_id TEXT NOT NULL,quantity DOUBLE NOT NULL,exact_quantity TEXT NOT NULL,unit TEXT NULL"),
        ("operator_activity", "action TEXT NOT NULL,action_id TEXT NULL,actor TEXT NULL,workstation TEXT NULL,role TEXT NULL,authorization_result TEXT NULL,reason TEXT NULL,context_references TEXT NULL"),
        ("audit_log", "action TEXT NOT NULL,target TEXT NOT NULL,actor TEXT NOT NULL,reason TEXT NOT NULL,authorization_result TEXT NULL,value_type TEXT NOT NULL,previous_value TEXT NOT NULL,current_value TEXT NOT NULL,workstation TEXT NULL"),
        ("electronic_signatures", "action_id TEXT NOT NULL,actor TEXT NOT NULL,meaning TEXT NOT NULL,authorization_result TEXT NULL,signed_source_id INT UNSIGNED NOT NULL,signed_sequence DECIMAL(20,0) NOT NULL"),
        ("system_events", "event_name TEXT NOT NULL,interval_ms DECIMAL(20,0) NULL,sequence_base DECIMAL(20,0) NULL,dropped_count DECIMAL(20,0) NULL,first_sequence DECIMAL(20,0) NULL,last_sequence DECIMAL(20,0) NULL,registered_source_id DECIMAL(20,0) NULL,previous_definition_hash TEXT NULL,new_definition_hash TEXT NULL,changed_epoch_id DECIMAL(20,0) NULL,clock_quality TEXT NULL,produced_count DECIMAL(20,0) NULL,cold_start BOOLEAN NULL"),
    ];
    for (table, fields) in tables {
        connection
            .query_drop(format!("CREATE TABLE IF NOT EXISTS `{table}` ({COMMON_DDL},{fields},PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)) ENGINE=InnoDB"))
            .map_err(error("create MySQL domain table"))?;
    }
    connection
        .query_drop(format!("CREATE TABLE IF NOT EXISTS data_loss ({COMMON_DDL},first_sequence DECIMAL(20,0) NOT NULL,last_sequence DECIMAL(20,0) NOT NULL,lost_count DECIMAL(20,0) NOT NULL,basis VARCHAR(16) NOT NULL,PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)) ENGINE=InnoDB"))
        .map_err(error("create MySQL data loss table"))?;
    connection
        .query_drop(format!("CREATE TABLE IF NOT EXISTS unresolved_records ({COMMON_DDL},event_type_id INT UNSIGNED NOT NULL,reason TEXT NOT NULL,diagnostic_summary TEXT NULL,PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)) ENGINE=InnoDB"))
        .map_err(error("create MySQL unresolved table"))?;
    Ok(())
}

pub(super) fn insert_projection(
    transaction: &mut Transaction<'_>,
    event: Option<EventLogRow>,
    values: Vec<LoggedValueRow>,
    domains: Vec<DomainRow>,
) -> Result<(), PersistenceError> {
    if let Some(event) = event {
        transaction.exec_drop(
            "INSERT INTO event_log(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields)
             VALUES(:record_id,STR_TO_DATE(LEFT(:event_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:event_time_ns,STR_TO_DATE(LEFT(:received_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:received_time_ns,:source,:source_id,:source_path,:source_hierarchy,:buffer_id,:run_id,:epoch_id,:sequence,:definition_hash,:time_unsynced,:synthetic_record,:partial_payload,:event_type_id,:event_name,:has_unclassified_fields)",
            common_params(&event, params! {
                "event_type_id" => event.event_type_id,
                "event_name" => &event.event_name,
                "has_unclassified_fields" => event.has_unclassified_fields,
            }),
        ).map_err(error("insert event projection"))?;
    }
    for value in values {
        let common = value.common;
        transaction.exec_drop(
            "INSERT INTO logged_values(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,value_id,value_name,value_type,unit,quality,semantic_role,boolean_value,signed_value,unsigned_value,number_value,text_value,exact_value)
             VALUES(:record_id,STR_TO_DATE(LEFT(:event_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:event_time_ns,STR_TO_DATE(LEFT(:received_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:received_time_ns,:source,:source_id,:source_path,:source_hierarchy,:buffer_id,:run_id,:epoch_id,:sequence,:definition_hash,:time_unsynced,:synthetic_record,:partial_payload,:value_id,:value_name,:value_type,:unit,:quality,:semantic_role,:boolean_value,:signed_value,:unsigned_value,:number_value,:text_value,:exact_value)",
            common_params(&common, params! {
                "value_id" => value.value_id,
                "value_name" => value.value_name,
                "value_type" => value.value_type,
                "unit" => value.unit,
                "quality" => value.quality,
                "semantic_role" => value.semantic_role,
                "boolean_value" => value.boolean_value,
                "signed_value" => value.signed_value,
                "unsigned_value" => value.unsigned_value,
                "number_value" => value.number_value,
                "text_value" => value.text_value,
                "exact_value" => value.exact_value,
            }),
        ).map_err(error("insert logged value projection"))?;
    }
    for domain in domains {
        match domain {
            DomainRow::Alarm(row) => {
            let common = row.common;
            transaction.exec_drop(
                "INSERT INTO alarm_history(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,`condition`,condition_class,lifecycle_action)
                 VALUES(:record_id,STR_TO_DATE(LEFT(:event_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:event_time_ns,STR_TO_DATE(LEFT(:received_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:received_time_ns,:source,:source_id,:source_path,:source_hierarchy,:buffer_id,:run_id,:epoch_id,:sequence,:definition_hash,:time_unsynced,:synthetic_record,:partial_payload,:condition,:condition_class,:lifecycle_action)",
                common_params(&common, params! {
                    "condition" => row.condition,
                    "condition_class" => row.condition_class,
                    "lifecycle_action" => row.lifecycle_action,
                }),
            ).map_err(error("insert alarm projection"))?;
            }
            DomainRow::Message(row) => {
                let args = message_args(&row.args);
                insert_event_domain(transaction, "message_log", "message_template,severity,severity_label,arg1_type,arg1_value,arg2_type,arg2_value,arg3_type,arg3_value,arg4_type,arg4_value", ":message_template,:severity,:severity_label,:arg1_type,:arg1_value,:arg2_type,:arg2_value,:arg3_type,:arg3_value,:arg4_type,:arg4_value", params! {
                    "record_id" => row.common.record_id, "message_template" => row.message_template,
                    "severity" => row.severity, "severity_label" => row.severity_label,
                    "arg1_type" => args[0].0, "arg1_value" => args[0].1,
                    "arg2_type" => args[1].0, "arg2_value" => args[1].1,
                    "arg3_type" => args[2].0, "arg3_value" => args[2].1,
                    "arg4_type" => args[3].0, "arg4_value" => args[3].1,
                })?;
            }
            DomainRow::State(row) => insert_event_domain(transaction, "state_history", "state_machine,state_category,previous_state,previous_state_label,new_state,new_state_label", ":state_machine,:state_category,:previous_state,:previous_state_label,:new_state,:new_state_label", params! {"record_id"=>row.common.record_id,"state_machine"=>row.state_machine,"state_category"=>row.state_category,"previous_state"=>row.previous_state,"previous_state_label"=>row.previous_state_label,"new_state"=>row.new_state,"new_state_label"=>row.new_state_label})?,
            DomainRow::Batch(row) => insert_event_domain(transaction, "batch_history", "batch_id,recipe_id,previous_state,new_state,new_state_label", ":batch_id,:recipe_id,:previous_state,:new_state,:new_state_label", params! {"record_id"=>row.common.record_id,"batch_id"=>row.batch_id,"recipe_id"=>row.recipe_id,"previous_state"=>row.previous_state,"new_state"=>row.new_state,"new_state_label"=>row.new_state_label})?,
            DomainRow::Recipe(row) => insert_event_domain(transaction, "recipe_history", "action,recipe_id,recipe_version,batch_id,actor,authorization_result", ":action,:recipe_id,:recipe_version,:batch_id,:actor,:authorization_result", params! {"record_id"=>row.common.record_id,"action"=>row.action,"recipe_id"=>row.recipe_id,"recipe_version"=>row.recipe_version,"batch_id"=>row.batch_id,"actor"=>row.actor,"authorization_result"=>row.authorization_result})?,
            DomainRow::Material(row) => insert_event_domain(transaction, "material_additions", "batch_id,material_id,quantity,exact_quantity,unit", ":batch_id,:material_id,:quantity,:exact_quantity,:unit", params! {"record_id"=>row.common.record_id,"batch_id"=>row.batch_id,"material_id"=>row.material_id,"quantity"=>row.quantity,"exact_quantity"=>row.exact_quantity,"unit"=>row.unit})?,
            DomainRow::Operator(row) => insert_event_domain(transaction, "operator_activity", "action,action_id,actor,workstation,role,authorization_result,reason,context_references", ":action,:action_id,:actor,:workstation,:role,:authorization_result,:reason,:context_references", params! {"record_id"=>row.common.record_id,"action"=>row.action,"action_id"=>row.action_id,"actor"=>row.actor,"workstation"=>row.workstation,"role"=>row.role,"authorization_result"=>row.authorization_result,"reason"=>row.reason,"context_references"=>row.context_references})?,
            DomainRow::Audit(row) => insert_event_domain(transaction, "audit_log", "action,target,actor,reason,authorization_result,value_type,previous_value,current_value,workstation", ":action,:target,:actor,:reason,:authorization_result,:value_type,:previous_value,:current_value,:workstation", params! {"record_id"=>row.common.record_id,"action"=>row.action,"target"=>row.target,"actor"=>row.actor,"reason"=>row.reason,"authorization_result"=>row.authorization_result,"value_type"=>row.value_type,"previous_value"=>row.previous_value,"current_value"=>row.current_value,"workstation"=>row.workstation})?,
            DomainRow::Signature(row) => insert_event_domain(transaction, "electronic_signatures", "action_id,actor,meaning,authorization_result,signed_source_id,signed_sequence", ":action_id,:actor,:meaning,:authorization_result,:signed_source_id,:signed_sequence", params! {"record_id"=>row.common.record_id,"action_id"=>row.action_id,"actor"=>row.actor,"meaning"=>row.meaning,"authorization_result"=>row.authorization_result,"signed_source_id"=>row.signed_source_id,"signed_sequence"=>row.signed_sequence})?,
            DomainRow::System(row) => insert_event_domain(transaction, "system_events", "event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start", ":event_name,:interval_ms,:sequence_base,:dropped_count,:first_sequence,:last_sequence,:registered_source_id,:previous_definition_hash,:new_definition_hash,:changed_epoch_id,:clock_quality,:produced_count,:cold_start", params! {"record_id"=>row.common.record_id,"event_name"=>row.event_name,"interval_ms"=>row.interval_ms,"sequence_base"=>row.sequence_base,"dropped_count"=>row.dropped_count,"first_sequence"=>row.first_sequence,"last_sequence"=>row.last_sequence,"registered_source_id"=>row.registered_source_id,"previous_definition_hash"=>row.previous_definition_hash,"new_definition_hash"=>row.new_definition_hash,"changed_epoch_id"=>row.changed_epoch_id,"clock_quality"=>row.clock_quality,"produced_count"=>row.produced_count,"cold_start"=>row.cold_start})?,
            DomainRow::Loss(row) => insert_loss(transaction, row)?,
            DomainRow::Unresolved(row) => insert_unresolved(transaction, row)?,
        }
    }
    Ok(())
}

pub(super) fn insert_event_batch(
    transaction: &mut Transaction<'_>,
    events: &[&EventLogRow],
) -> Result<(), PersistenceError> {
    if events.is_empty() {
        return Ok(());
    }
    let tuple = "(?,STR_TO_DATE(LEFT(?,26),'%Y-%m-%dT%H:%i:%s.%f'),?,STR_TO_DATE(LEFT(?,26),'%Y-%m-%dT%H:%i:%s.%f'),?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)";
    let placeholders = std::iter::repeat_n(tuple, events.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut values = Vec::with_capacity(events.len() * 20);
    for event in events {
        values.extend([
            mysql::Value::from(event.record_id.as_str()),
            mysql::Value::from(event.event_time.clone()),
            mysql::Value::from(event.event_time_ns.clone()),
            mysql::Value::from(event.received_time.as_str()),
            mysql::Value::from(event.received_time_ns.as_str()),
            mysql::Value::from(event.source.clone()),
            mysql::Value::from(event.source_id),
            mysql::Value::from(event.source_path.as_str()),
            mysql::Value::from(event.source_hierarchy.as_str()),
            mysql::Value::from(event.buffer_id),
            mysql::Value::from(event.run_id.as_str()),
            mysql::Value::from(event.epoch_id.as_str()),
            mysql::Value::from(event.sequence.as_str()),
            mysql::Value::from(event.definition_hash.as_str()),
            mysql::Value::from(event.time_unsynced),
            mysql::Value::from(event.synthetic_record),
            mysql::Value::from(event.partial_payload),
            mysql::Value::from(event.event_type_id),
            mysql::Value::from(event.event_name.as_str()),
            mysql::Value::from(event.has_unclassified_fields),
        ]);
    }
    transaction.exec_drop(
        format!("INSERT INTO event_log(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields) VALUES {placeholders}"),
        mysql::Params::Positional(values),
    ).map_err(error("insert event projection batch"))
}

fn insert_event_domain(
    transaction: &mut Transaction<'_>,
    table: &str,
    columns: &str,
    values: &str,
    params: mysql::Params,
) -> Result<(), PersistenceError> {
    transaction
        .exec_drop(
            format!("INSERT INTO `{table}` ({COMMON_COLUMNS},{columns}) SELECT {COMMON_SELECT},{values} FROM event_log e WHERE e.record_id=:record_id"),
            params,
        )
        .map_err(error("insert MySQL domain projection"))
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
    row: super::projection_domains::LossRow,
) -> Result<(), PersistenceError> {
    transaction.exec_drop(
        "INSERT INTO data_loss(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,first_sequence,last_sequence,lost_count,basis)
         VALUES(:record_id,NULL,NULL,STR_TO_DATE(LEFT(:received_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:received_time_ns,:source,:source_id,:source_path,:source_hierarchy,:buffer_id,:run_id,:epoch_id,:first_sequence,:definition_hash,:time_unsynced,:synthetic_record,:partial_payload,:first_sequence,:last_sequence,:lost_count,:basis)",
        params! {"record_id"=>row.record_id,"received_time"=>row.received_time,"received_time_ns"=>row.received_time_ns,"source"=>row.source,"source_id"=>row.source_id,"source_path"=>row.source_path,"source_hierarchy"=>row.source_hierarchy,"buffer_id"=>row.buffer_id,"run_id"=>row.run_id,"epoch_id"=>row.epoch_id,"definition_hash"=>row.definition_hash,"time_unsynced"=>row.time_unsynced,"synthetic_record"=>row.synthetic_record,"partial_payload"=>row.partial_payload,"first_sequence"=>row.first_sequence,"last_sequence"=>row.last_sequence,"lost_count"=>row.lost_count,"basis"=>row.basis},
    ).map_err(error("insert MySQL data loss projection"))
}

fn insert_unresolved(
    transaction: &mut Transaction<'_>,
    row: super::projection_domains::UnresolvedRow,
) -> Result<(), PersistenceError> {
    transaction.exec_drop(
        "INSERT INTO unresolved_records(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,reason,diagnostic_summary)
         VALUES(:record_id,STR_TO_DATE(LEFT(:event_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:event_time_ns,STR_TO_DATE(LEFT(:received_time,26),'%Y-%m-%dT%H:%i:%s.%f'),:received_time_ns,:source,:source_id,:source_path,:source_hierarchy,:buffer_id,:run_id,:epoch_id,:sequence,:definition_hash,:time_unsynced,:synthetic_record,:partial_payload,:event_type_id,:reason,:diagnostic_summary)",
        params! {"record_id"=>row.record_id,"event_time"=>row.event_time,"event_time_ns"=>row.event_time_ns,"received_time"=>row.received_time,"received_time_ns"=>row.received_time_ns,"source"=>row.source,"source_id"=>row.source_id,"source_path"=>row.source_path,"source_hierarchy"=>row.source_hierarchy,"buffer_id"=>row.buffer_id,"run_id"=>row.run_id,"epoch_id"=>row.epoch_id,"sequence"=>row.sequence,"definition_hash"=>row.definition_hash,"time_unsynced"=>row.time_unsynced,"synthetic_record"=>row.synthetic_record,"partial_payload"=>row.partial_payload,"event_type_id"=>row.event_type_id,"reason"=>row.reason,"diagnostic_summary"=>row.diagnostic_summary},
    ).map_err(error("insert MySQL unresolved projection"))
}

fn common_params(event: &EventLogRow, mut extra: mysql::Params) -> mysql::Params {
    let mysql::Params::Named(ref mut values) = extra else {
        unreachable!("named MySQL projection parameters")
    };
    let common = params! {
        "record_id" => &event.record_id,
        "event_time" => &event.event_time,
        "event_time_ns" => &event.event_time_ns,
        "received_time" => &event.received_time,
        "received_time_ns" => &event.received_time_ns,
        "source" => &event.source,
        "source_id" => event.source_id,
        "source_path" => &event.source_path,
        "source_hierarchy" => &event.source_hierarchy,
        "buffer_id" => event.buffer_id,
        "run_id" => &event.run_id,
        "epoch_id" => &event.epoch_id,
        "sequence" => &event.sequence,
        "definition_hash" => &event.definition_hash,
        "time_unsynced" => event.time_unsynced,
        "synthetic_record" => event.synthetic_record,
        "partial_payload" => event.partial_payload,
    };
    let mysql::Params::Named(common) = common else {
        unreachable!("named MySQL common parameters")
    };
    values.extend(common);
    extra
}

fn error(context: &'static str) -> impl FnOnce(mysql::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("MySQL {context}: {error}"))
}
