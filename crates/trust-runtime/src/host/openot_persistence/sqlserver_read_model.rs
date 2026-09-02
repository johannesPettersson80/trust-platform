use tiberius::{Client, Query};
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use super::projection::{EventLogRow, LoggedValueRow, ProjectedDocument};
use super::projection_domains::DomainRow;
use super::PersistenceError;

type TdsClient = Client<Compat<TcpStream>>;

const COMMON_COLUMNS: &str = "record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload";
const COMMON_SELECT: &str = "e.record_id,e.event_time,e.event_time_ns,e.received_time,e.received_time_ns,e.source,e.source_id,e.source_path,e.source_hierarchy,e.buffer_id,e.run_id,e.epoch_id,e.sequence,e.definition_hash,e.time_unsynced,e.synthetic_record,e.partial_payload";
const COMMON_DDL: &str = "record_id NVARCHAR(255) COLLATE Latin1_General_100_BIN2 NOT NULL,event_time DATETIME2(7) NULL,event_time_ns DECIMAL(20,0) NULL,received_time DATETIME2(7) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,source NVARCHAR(MAX) NULL,source_id BIGINT NOT NULL,source_path NVARCHAR(MAX) NOT NULL,source_hierarchy NVARCHAR(MAX) NOT NULL,buffer_id BIGINT NOT NULL,run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,sequence DECIMAL(20,0) NOT NULL,definition_hash NVARCHAR(MAX) NOT NULL,time_unsynced BIT NOT NULL,synthetic_record BIT NOT NULL,partial_payload BIT NOT NULL";
const SQLSERVER_LOGGED_VALUE_ROWS_PER_STATEMENT: usize = 53;

pub(super) fn create_domain_schema(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
) -> Result<(), PersistenceError> {
    let tables = [
        ("message_log", "message_template NVARCHAR(MAX) NOT NULL,severity INT NULL,severity_label NVARCHAR(MAX) NULL,arg1_type NVARCHAR(MAX) NULL,arg1_value NVARCHAR(MAX) NULL,arg2_type NVARCHAR(MAX) NULL,arg2_value NVARCHAR(MAX) NULL,arg3_type NVARCHAR(MAX) NULL,arg3_value NVARCHAR(MAX) NULL,arg4_type NVARCHAR(MAX) NULL,arg4_value NVARCHAR(MAX) NULL"),
        ("state_history", "state_machine NVARCHAR(MAX) NOT NULL,state_category NVARCHAR(MAX) NOT NULL,previous_state NVARCHAR(MAX) NOT NULL,previous_state_label NVARCHAR(MAX) NULL,new_state NVARCHAR(MAX) NOT NULL,new_state_label NVARCHAR(MAX) NULL"),
        ("batch_history", "batch_id NVARCHAR(MAX) NOT NULL,recipe_id NVARCHAR(MAX) NULL,previous_state NVARCHAR(MAX) NULL,new_state NVARCHAR(MAX) NOT NULL,new_state_label NVARCHAR(MAX) NULL"),
        ("recipe_history", "action NVARCHAR(MAX) NOT NULL,recipe_id NVARCHAR(MAX) NOT NULL,recipe_version NVARCHAR(MAX) NULL,batch_id NVARCHAR(MAX) NULL,actor NVARCHAR(MAX) NULL,authorization_result NVARCHAR(MAX) NULL"),
        ("material_additions", "batch_id NVARCHAR(MAX) NOT NULL,material_id NVARCHAR(MAX) NOT NULL,quantity FLOAT NOT NULL,exact_quantity NVARCHAR(MAX) NOT NULL,unit NVARCHAR(MAX) NULL"),
        ("operator_activity", "action NVARCHAR(MAX) NOT NULL,action_id NVARCHAR(MAX) NULL,actor NVARCHAR(MAX) NULL,workstation NVARCHAR(MAX) NULL,role NVARCHAR(MAX) NULL,authorization_result NVARCHAR(MAX) NULL,reason NVARCHAR(MAX) NULL,context_references NVARCHAR(MAX) NULL"),
        ("audit_log", "action NVARCHAR(MAX) NOT NULL,target NVARCHAR(MAX) NOT NULL,actor NVARCHAR(MAX) NOT NULL,reason NVARCHAR(MAX) NOT NULL,authorization_result NVARCHAR(MAX) NULL,value_type NVARCHAR(MAX) NOT NULL,previous_value NVARCHAR(MAX) NOT NULL,current_value NVARCHAR(MAX) NOT NULL,workstation NVARCHAR(MAX) NULL"),
        ("electronic_signatures", "action_id NVARCHAR(MAX) NOT NULL,actor NVARCHAR(MAX) NOT NULL,meaning NVARCHAR(MAX) NOT NULL,authorization_result NVARCHAR(MAX) NULL,signed_source_id BIGINT NOT NULL,signed_sequence DECIMAL(20,0) NOT NULL"),
        ("system_events", "event_name NVARCHAR(MAX) NOT NULL,interval_ms DECIMAL(20,0) NULL,sequence_base DECIMAL(20,0) NULL,dropped_count DECIMAL(20,0) NULL,first_sequence DECIMAL(20,0) NULL,last_sequence DECIMAL(20,0) NULL,registered_source_id DECIMAL(20,0) NULL,previous_definition_hash NVARCHAR(MAX) NULL,new_definition_hash NVARCHAR(MAX) NULL,changed_epoch_id DECIMAL(20,0) NULL,clock_quality NVARCHAR(MAX) NULL,produced_count DECIMAL(20,0) NULL,cold_start BIT NULL"),
    ];
    for (table, fields) in tables {
        batch(runtime, client, &format!("IF OBJECT_ID(N'[{schema}].{table}',N'U') IS NULL CREATE TABLE [{schema}].{table}({COMMON_DDL},{fields},PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES [{schema}].logging_records(identity_key))"))?;
    }
    batch(runtime, client, &format!("IF OBJECT_ID(N'[{schema}].data_loss',N'U') IS NULL CREATE TABLE [{schema}].data_loss({COMMON_DDL},first_sequence DECIMAL(20,0) NOT NULL,last_sequence DECIMAL(20,0) NOT NULL,lost_count DECIMAL(20,0) NOT NULL,basis NVARCHAR(16) NOT NULL,PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES [{schema}].logging_records(identity_key)); IF OBJECT_ID(N'[{schema}].unresolved_records',N'U') IS NULL CREATE TABLE [{schema}].unresolved_records({COMMON_DDL},event_type_id BIGINT NOT NULL,reason NVARCHAR(MAX) NOT NULL,diagnostic_summary NVARCHAR(MAX) NULL,PRIMARY KEY(record_id),FOREIGN KEY(record_id) REFERENCES [{schema}].logging_records(identity_key))"))
}

pub(super) fn insert_projection(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    event: Option<EventLogRow>,
    values: Vec<LoggedValueRow>,
    domains: Vec<DomainRow>,
) -> Result<(), PersistenceError> {
    if let Some(event) = event {
        insert_event(runtime, client, schema, &event)?;
    }
    for value in values {
        insert_value(runtime, client, schema, value)?;
    }
    macro_rules! event_domain {
        ($table:literal, $record_id:expr, $columns:literal, $values:literal $(, $value:expr)* $(,)?) => {{
            let mut query = Query::new(format!(
                "INSERT INTO [{schema}].{} ({COMMON_COLUMNS},{}) SELECT {COMMON_SELECT},{} FROM [{schema}].event_log e WHERE e.record_id=@P1",
                $table, $columns, $values
            ));
            query.bind($record_id);
            $(query.bind($value);)*
            execute(runtime, client, query, "insert domain projection")?;
        }};
    }
    for domain in domains {
        match domain {
        DomainRow::Alarm(row) => {
            let common = row.common;
            let mut query = Query::new(format!(
                "INSERT INTO [{schema}].alarm_history(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,condition,condition_class,lifecycle_action)
                 VALUES(@P1,CONVERT(DATETIME2(7),LEFT(@P2,27),127),CONVERT(DECIMAL(20,0),@P3),CONVERT(DATETIME2(7),LEFT(@P4,27),127),CONVERT(DECIMAL(20,0),@P5),@P6,@P7,@P8,@P9,@P10,CONVERT(DECIMAL(20,0),@P11),CONVERT(DECIMAL(20,0),@P12),CONVERT(DECIMAL(20,0),@P13),@P14,@P15,@P16,@P17,@P18,@P19,@P20)"
            ));
            bind_common(&mut query, &common);
            query.bind(row.condition);
            query.bind(row.condition_class);
            query.bind(row.lifecycle_action);
            execute(runtime, client, query, "insert alarm projection")?;
        }
        DomainRow::Message(row) => {
            let args = message_args(&row.args);
            event_domain!("message_log", row.common.record_id, "message_template,severity,severity_label,arg1_type,arg1_value,arg2_type,arg2_value,arg3_type,arg3_value,arg4_type,arg4_value", "@P2,@P3,@P4,@P5,@P6,@P7,@P8,@P9,@P10,@P11,@P12", row.message_template, row.severity.map(i32::from), row.severity_label, args[0].0, args[0].1, args[1].0, args[1].1, args[2].0, args[2].1, args[3].0, args[3].1);
        }
        DomainRow::State(row) => event_domain!("state_history", row.common.record_id, "state_machine,state_category,previous_state,previous_state_label,new_state,new_state_label", "@P2,@P3,@P4,@P5,@P6,@P7", row.state_machine, row.state_category, row.previous_state, row.previous_state_label, row.new_state, row.new_state_label),
        DomainRow::Batch(row) => event_domain!("batch_history", row.common.record_id, "batch_id,recipe_id,previous_state,new_state,new_state_label", "@P2,@P3,@P4,@P5,@P6", row.batch_id, row.recipe_id, row.previous_state, row.new_state, row.new_state_label),
        DomainRow::Recipe(row) => event_domain!("recipe_history", row.common.record_id, "action,recipe_id,recipe_version,batch_id,actor,authorization_result", "@P2,@P3,@P4,@P5,@P6,@P7", row.action, row.recipe_id, row.recipe_version, row.batch_id, row.actor, row.authorization_result),
        DomainRow::Material(row) => event_domain!("material_additions", row.common.record_id, "batch_id,material_id,quantity,exact_quantity,unit", "@P2,@P3,@P4,@P5,@P6", row.batch_id, row.material_id, row.quantity, row.exact_quantity, row.unit),
        DomainRow::Operator(row) => event_domain!("operator_activity", row.common.record_id, "action,action_id,actor,workstation,role,authorization_result,reason,context_references", "@P2,@P3,@P4,@P5,@P6,@P7,@P8,@P9", row.action, row.action_id, row.actor, row.workstation, row.role, row.authorization_result, row.reason, row.context_references),
        DomainRow::Audit(row) => event_domain!("audit_log", row.common.record_id, "action,target,actor,reason,authorization_result,value_type,previous_value,current_value,workstation", "@P2,@P3,@P4,@P5,@P6,@P7,@P8,@P9,@P10", row.action, row.target, row.actor, row.reason, row.authorization_result, row.value_type, row.previous_value, row.current_value, row.workstation),
        DomainRow::Signature(row) => event_domain!("electronic_signatures", row.common.record_id, "action_id,actor,meaning,authorization_result,signed_source_id,signed_sequence", "@P2,@P3,@P4,@P5,@P6,CONVERT(DECIMAL(20,0),@P7)", row.action_id, row.actor, row.meaning, row.authorization_result, i64::from(row.signed_source_id), row.signed_sequence),
        DomainRow::System(row) => event_domain!("system_events", row.common.record_id, "event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start", "@P2,CONVERT(DECIMAL(20,0),@P3),CONVERT(DECIMAL(20,0),@P4),CONVERT(DECIMAL(20,0),@P5),CONVERT(DECIMAL(20,0),@P6),CONVERT(DECIMAL(20,0),@P7),CONVERT(DECIMAL(20,0),@P8),@P9,@P10,CONVERT(DECIMAL(20,0),@P11),@P12,CONVERT(DECIMAL(20,0),@P13),@P14", row.event_name, row.interval_ms.map(|v| v.to_string()), row.sequence_base, row.dropped_count, row.first_sequence, row.last_sequence, row.registered_source_id, row.previous_definition_hash, row.new_definition_hash, row.changed_epoch_id, row.clock_quality, row.produced_count, row.cold_start),
        DomainRow::Loss(row) => insert_loss(runtime, client, schema, &row)?,
        DomainRow::Unresolved(row) => insert_unresolved(runtime, client, schema, &row)?,
        }
    }
    Ok(())
}

pub(super) fn insert_event_batch(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    events: &[&EventLogRow],
) -> Result<(), PersistenceError> {
    if events.is_empty() {
        return Ok(());
    }
    let mut parameter = 1;
    let tuples = events
        .iter()
        .map(|_| {
            let mut next = || {
                let value = parameter;
                parameter += 1;
                value
            };
            let p1 = next();
            let p2 = next();
            let p3 = next();
            let p4 = next();
            let p5 = next();
            let rest = (0..15).map(|_| next()).collect::<Vec<_>>();
            format!("(@P{p1},CONVERT(DATETIME2(7),LEFT(@P{p2},27),127),CONVERT(DECIMAL(20,0),@P{p3}),CONVERT(DATETIME2(7),LEFT(@P{p4},27),127),CONVERT(DECIMAL(20,0),@P{p5}),{})", rest.iter().map(|index| format!("@P{index}")).collect::<Vec<_>>().join(","))
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut query = Query::new(format!(
        "INSERT INTO [{schema}].event_log({COMMON_COLUMNS},event_type_id,event_name,has_unclassified_fields) VALUES {tuples}"
    ));
    for event in events {
        bind_common(&mut query, event);
        query.bind(i64::from(event.event_type_id));
        query.bind(event.event_name.as_str());
        query.bind(event.has_unclassified_fields);
    }
    execute(runtime, client, query, "insert event projection batch")
}

pub(super) fn insert_value_batch(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    values: &[&LoggedValueRow],
) -> Result<(), PersistenceError> {
    if values.is_empty() {
        return Ok(());
    }
    for values in values.chunks(SQLSERVER_LOGGED_VALUE_ROWS_PER_STATEMENT) {
        insert_value_batch_chunk(runtime, client, schema, values)?;
    }
    Ok(())
}

fn insert_value_batch_chunk(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    values: &[&LoggedValueRow],
) -> Result<(), PersistenceError> {
    let mut parameter = 1;
    let tuples = values
        .iter()
        .map(|_| {
            let indices = (0..39)
                .map(|_| {
                    let value = parameter;
                    parameter += 1;
                    value
                })
                .collect::<Vec<_>>();
            let mut raw = indices
                .iter()
                .map(|index| format!("@P{index}"))
                .collect::<Vec<_>>();
            raw[1] = format!("CONVERT(DATETIME2(7),LEFT({},27),127)", raw[1]);
            raw[2] = format!("CONVERT(DECIMAL(20,0),{})", raw[2]);
            raw[3] = format!("CONVERT(DATETIME2(7),LEFT({},27),127)", raw[3]);
            raw[4] = format!("CONVERT(DECIMAL(20,0),{})", raw[4]);
            for index in [10_usize, 11, 12, 25, 31] {
                raw[index] = format!("CONVERT(DECIMAL(20,0),{})", raw[index]);
            }
            format!("({})", raw.join(","))
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut query = Query::new(format!("INSERT INTO [{schema}].logged_values(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,value_id,value_name,value_type,unit,quality,semantic_role,boolean_value,signed_value,unsigned_value,number_value,text_value,exact_value,previous_boolean_value,previous_signed_value,previous_unsigned_value,previous_number_value,previous_text_value,previous_exact_value,is_audited,actor,reason,authorization_result) VALUES {tuples}"));
    for value in values {
        bind_common(&mut query, &value.common);
        query.bind(i64::from(value.value_id));
        query.bind(value.value_name.as_str());
        query.bind(value.value_type);
        query.bind(value.unit.as_deref());
        query.bind(value.quality.map(i32::from));
        query.bind(i32::from(value.semantic_role));
        query.bind(value.boolean_value);
        query.bind(value.signed_value);
        query.bind(value.unsigned_value.as_deref());
        query.bind(value.number_value);
        query.bind(value.text_value.as_deref());
        query.bind(value.exact_value.as_str());
        query.bind(value.previous_boolean_value);
        query.bind(value.previous_signed_value);
        query.bind(value.previous_unsigned_value.as_deref());
        query.bind(value.previous_number_value);
        query.bind(value.previous_text_value.as_deref());
        query.bind(value.previous_exact_value.as_deref());
        query.bind(value.is_audited);
        query.bind(value.actor.as_deref());
        query.bind(value.reason.as_deref());
        query.bind(value.authorization_result.as_deref());
    }
    execute(
        runtime,
        client,
        query,
        "insert logged value projection batch",
    )
}

pub(super) fn insert_singleton_event_domains_batch(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    documents: &[ProjectedDocument],
) -> Result<(), PersistenceError> {
    let domains = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter(|domain| {
            matches!(
                domain,
                DomainRow::Message(_)
                    | DomainRow::State(_)
                    | DomainRow::Batch(_)
                    | DomainRow::Material(_)
                    | DomainRow::Audit(_)
                    | DomainRow::Signature(_)
            )
        })
        .collect::<Vec<_>>();
    if domains.is_empty() {
        return Ok(());
    }
    let mut parameter = 1;
    let mut statements = Vec::new();
    for domain in &domains {
        let (table, columns, count, conversions) = match domain {
            DomainRow::Message(_) => ("message_log", "message_template,severity,severity_label,arg1_type,arg1_value,arg2_type,arg2_value,arg3_type,arg3_value,arg4_type,arg4_value", 12, None),
            DomainRow::State(_) => ("state_history", "state_machine,state_category,previous_state,previous_state_label,new_state,new_state_label", 7, None),
            DomainRow::Batch(_) => ("batch_history", "batch_id,recipe_id,previous_state,new_state,new_state_label", 6, None),
            DomainRow::Material(_) => ("material_additions", "batch_id,material_id,quantity,exact_quantity,unit", 6, None),
            DomainRow::Audit(_) => ("audit_log", "action,target,actor,reason,authorization_result,value_type,previous_value,current_value,workstation", 10, None),
            DomainRow::Signature(_) => ("electronic_signatures", "action_id,actor,meaning,authorization_result,signed_source_id,signed_sequence", 7, Some(6)),
            _ => unreachable!(),
        };
        let start = parameter;
        parameter += count;
        let values = (start + 1..start + count)
            .map(|index| {
                if conversions == Some(index - start) {
                    format!("CONVERT(DECIMAL(20,0),@P{index})")
                } else {
                    format!("@P{index}")
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        statements.push(format!("INSERT INTO [{schema}].{table}({COMMON_COLUMNS},{columns}) SELECT {COMMON_SELECT},{values} FROM [{schema}].event_log e WHERE e.record_id=@P{start}"));
    }
    let mut query = Query::new(statements.join(";"));
    for domain in domains {
        match domain {
            DomainRow::Message(row) => {
                let args = message_args(&row.args);
                query.bind(row.common.record_id.as_str());
                query.bind(row.message_template.as_str());
                query.bind(row.severity.map(i32::from));
                query.bind(row.severity_label.as_deref());
                for (kind, value) in args {
                    query.bind(kind);
                    query.bind(value);
                }
            }
            DomainRow::State(row) => {
                query.bind(row.common.record_id.as_str());
                query.bind(row.state_machine.as_str());
                query.bind(row.state_category.as_str());
                query.bind(row.previous_state.as_str());
                query.bind(row.previous_state_label.as_deref());
                query.bind(row.new_state.as_str());
                query.bind(row.new_state_label.as_deref());
            }
            DomainRow::Batch(row) => {
                query.bind(row.common.record_id.as_str());
                query.bind(row.batch_id.as_str());
                query.bind(row.recipe_id.as_deref());
                query.bind(row.previous_state.as_deref());
                query.bind(row.new_state.as_str());
                query.bind(row.new_state_label.as_deref());
            }
            DomainRow::Material(row) => {
                query.bind(row.common.record_id.as_str());
                query.bind(row.batch_id.as_str());
                query.bind(row.material_id.as_str());
                query.bind(row.quantity);
                query.bind(row.exact_quantity.as_str());
                query.bind(row.unit.as_deref());
            }
            DomainRow::Audit(row) => {
                query.bind(row.common.record_id.as_str());
                query.bind(row.action.as_str());
                query.bind(row.target.as_str());
                query.bind(row.actor.as_str());
                query.bind(row.reason.as_str());
                query.bind(row.authorization_result.as_deref());
                query.bind(row.value_type.as_str());
                query.bind(row.previous_value.as_str());
                query.bind(row.current_value.as_str());
                query.bind(row.workstation.as_deref());
            }
            DomainRow::Signature(row) => {
                query.bind(row.common.record_id.as_str());
                query.bind(row.action_id.as_str());
                query.bind(row.actor.as_str());
                query.bind(row.meaning.as_str());
                query.bind(row.authorization_result.as_deref());
                query.bind(i64::from(row.signed_source_id));
                query.bind(row.signed_sequence.as_str());
            }
            _ => unreachable!(),
        }
    }
    execute(
        runtime,
        client,
        query,
        "insert singleton event domains batch",
    )
}

pub(super) fn insert_repeated_domains_combined(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
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
    let systems = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::System(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    let operators = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::Operator(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    let recipes = documents
        .iter()
        .flat_map(|document| document.domains.iter())
        .filter_map(|domain| match domain {
            DomainRow::Recipe(row) => Some(row),
            _ => None,
        })
        .collect::<Vec<_>>();
    if alarms.is_empty() && systems.is_empty() && operators.is_empty() && recipes.is_empty() {
        return Ok(());
    }
    fn tuples(count: usize, width: usize, parameter: &mut usize) -> String {
        (0..count)
            .map(|_| {
                let values = (0..width)
                    .map(|_| {
                        let value = format!("@P{}", *parameter);
                        *parameter += 1;
                        value
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("({values})")
            })
            .collect::<Vec<_>>()
            .join(",")
    }
    let mut parameter = 1;
    let alarm_values = tuples(alarms.len(), 4, &mut parameter);
    let system_values = tuples(systems.len(), 14, &mut parameter);
    let operator_values = tuples(operators.len(), 9, &mut parameter);
    let recipe_values = tuples(recipes.len(), 7, &mut parameter);
    let mut statements = Vec::new();
    if !alarms.is_empty() {
        statements.push(format!("INSERT INTO [{schema}].alarm_history({COMMON_COLUMNS},condition,condition_class,lifecycle_action) SELECT {COMMON_SELECT},rows.condition,rows.condition_class,rows.lifecycle_action FROM [{schema}].event_log e JOIN (VALUES {alarm_values}) rows(record_id,condition,condition_class,lifecycle_action) ON e.record_id=rows.record_id"));
    }
    if !systems.is_empty() {
        statements.push(format!("INSERT INTO [{schema}].system_events({COMMON_COLUMNS},event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start) SELECT {COMMON_SELECT},rows.event_name,CONVERT(DECIMAL(20,0),rows.interval_ms),CONVERT(DECIMAL(20,0),rows.sequence_base),CONVERT(DECIMAL(20,0),rows.dropped_count),CONVERT(DECIMAL(20,0),rows.first_sequence),CONVERT(DECIMAL(20,0),rows.last_sequence),CONVERT(DECIMAL(20,0),rows.registered_source_id),rows.previous_definition_hash,rows.new_definition_hash,CONVERT(DECIMAL(20,0),rows.changed_epoch_id),rows.clock_quality,CONVERT(DECIMAL(20,0),rows.produced_count),rows.cold_start FROM [{schema}].event_log e JOIN (VALUES {system_values}) rows(record_id,event_name,interval_ms,sequence_base,dropped_count,first_sequence,last_sequence,registered_source_id,previous_definition_hash,new_definition_hash,changed_epoch_id,clock_quality,produced_count,cold_start) ON e.record_id=rows.record_id"));
    }
    if !operators.is_empty() {
        statements.push(format!("INSERT INTO [{schema}].operator_activity({COMMON_COLUMNS},action,action_id,actor,workstation,role,authorization_result,reason,context_references) SELECT {COMMON_SELECT},rows.action,rows.action_id,rows.actor,rows.workstation,rows.role,rows.authorization_result,rows.reason,rows.context_references FROM [{schema}].event_log e JOIN (VALUES {operator_values}) rows(record_id,action,action_id,actor,workstation,role,authorization_result,reason,context_references) ON e.record_id=rows.record_id"));
    }
    if !recipes.is_empty() {
        statements.push(format!("INSERT INTO [{schema}].recipe_history({COMMON_COLUMNS},action,recipe_id,recipe_version,batch_id,actor,authorization_result) SELECT {COMMON_SELECT},rows.action,rows.recipe_id,rows.recipe_version,rows.batch_id,rows.actor,rows.authorization_result FROM [{schema}].event_log e JOIN (VALUES {recipe_values}) rows(record_id,action,recipe_id,recipe_version,batch_id,actor,authorization_result) ON e.record_id=rows.record_id"));
    }
    let intervals = systems
        .iter()
        .map(|row| row.interval_ms.map(|value| value.to_string()))
        .collect::<Vec<_>>();
    let mut query = Query::new(statements.join(";"));
    for row in alarms {
        query.bind(row.common.record_id.as_str());
        query.bind(row.condition.as_str());
        query.bind(row.condition_class.as_deref());
        query.bind(row.lifecycle_action.as_str());
    }
    for (row, interval) in systems.into_iter().zip(intervals.iter()) {
        query.bind(row.common.record_id.as_str());
        query.bind(row.event_name.as_str());
        query.bind(interval.as_deref());
        query.bind(row.sequence_base.as_deref());
        query.bind(row.dropped_count.as_deref());
        query.bind(row.first_sequence.as_deref());
        query.bind(row.last_sequence.as_deref());
        query.bind(row.registered_source_id.as_deref());
        query.bind(row.previous_definition_hash.as_deref());
        query.bind(row.new_definition_hash.as_deref());
        query.bind(row.changed_epoch_id.as_deref());
        query.bind(row.clock_quality.as_deref());
        query.bind(row.produced_count.as_deref());
        query.bind(row.cold_start);
    }
    for row in operators {
        query.bind(row.common.record_id.as_str());
        query.bind(row.action.as_str());
        query.bind(row.action_id.as_deref());
        query.bind(row.actor.as_deref());
        query.bind(row.workstation.as_deref());
        query.bind(row.role.as_deref());
        query.bind(row.authorization_result.as_deref());
        query.bind(row.reason.as_deref());
        query.bind(row.context_references.as_deref());
    }
    for row in recipes {
        query.bind(row.common.record_id.as_str());
        query.bind(row.action.as_str());
        query.bind(row.recipe_id.as_str());
        query.bind(row.recipe_version.as_deref());
        query.bind(row.batch_id.as_deref());
        query.bind(row.actor.as_deref());
        query.bind(row.authorization_result.as_deref());
    }
    execute(runtime, client, query, "insert repeated domains combined")
}

pub(super) fn insert_loss_and_unresolved_batch(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    documents: &[ProjectedDocument],
) -> Result<(), PersistenceError> {
    for domain in documents
        .iter()
        .flat_map(|document| document.domains.iter())
    {
        match domain {
            DomainRow::Loss(row) => insert_loss(runtime, client, schema, row)?,
            DomainRow::Unresolved(row) => insert_unresolved(runtime, client, schema, row)?,
            _ => {}
        }
    }
    Ok(())
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
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    row: &super::projection_domains::LossRow,
) -> Result<(), PersistenceError> {
    let mut query = Query::new(format!("INSERT INTO [{schema}].data_loss({COMMON_COLUMNS},first_sequence,last_sequence,lost_count,basis) VALUES(@P1,NULL,NULL,CONVERT(DATETIME2(7),LEFT(@P2,27),127),CONVERT(DECIMAL(20,0),@P3),@P4,@P5,@P6,@P7,@P8,CONVERT(DECIMAL(20,0),@P9),CONVERT(DECIMAL(20,0),@P10),CONVERT(DECIMAL(20,0),@P11),@P12,@P13,@P14,@P15,CONVERT(DECIMAL(20,0),@P11),CONVERT(DECIMAL(20,0),@P16),CONVERT(DECIMAL(20,0),@P17),@P18)"));
    query.bind(row.record_id.as_str());
    query.bind(row.received_time.as_str());
    query.bind(row.received_time_ns.as_str());
    query.bind(row.source.as_deref());
    query.bind(i64::from(row.source_id));
    query.bind(row.source_path.as_str());
    query.bind(row.source_hierarchy.as_str());
    query.bind(i64::from(row.buffer_id));
    query.bind(row.run_id.as_str());
    query.bind(row.epoch_id.as_str());
    query.bind(row.first_sequence.as_str());
    query.bind(row.definition_hash.as_str());
    query.bind(row.time_unsynced);
    query.bind(row.synthetic_record);
    query.bind(row.partial_payload);
    query.bind(row.last_sequence.as_str());
    query.bind(row.lost_count.as_str());
    query.bind(row.basis);
    execute(runtime, client, query, "insert data loss projection")
}

fn insert_unresolved(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    row: &super::projection_domains::UnresolvedRow,
) -> Result<(), PersistenceError> {
    let mut query = Query::new(format!("INSERT INTO [{schema}].unresolved_records({COMMON_COLUMNS},event_type_id,reason,diagnostic_summary) VALUES(@P1,CONVERT(DATETIME2(7),LEFT(@P2,27),127),CONVERT(DECIMAL(20,0),@P3),CONVERT(DATETIME2(7),LEFT(@P4,27),127),CONVERT(DECIMAL(20,0),@P5),@P6,@P7,@P8,@P9,@P10,CONVERT(DECIMAL(20,0),@P11),CONVERT(DECIMAL(20,0),@P12),CONVERT(DECIMAL(20,0),@P13),@P14,@P15,@P16,@P17,@P18,@P19,@P20)"));
    query.bind(row.record_id.as_str());
    query.bind(row.event_time.as_deref());
    query.bind(row.event_time_ns.as_deref());
    query.bind(row.received_time.as_str());
    query.bind(row.received_time_ns.as_str());
    query.bind(row.source.as_deref());
    query.bind(i64::from(row.source_id));
    query.bind(row.source_path.as_str());
    query.bind(row.source_hierarchy.as_str());
    query.bind(i64::from(row.buffer_id));
    query.bind(row.run_id.as_str());
    query.bind(row.epoch_id.as_str());
    query.bind(row.sequence.as_str());
    query.bind(row.definition_hash.as_str());
    query.bind(row.time_unsynced);
    query.bind(row.synthetic_record);
    query.bind(row.partial_payload);
    query.bind(i64::from(row.event_type_id));
    query.bind(row.reason.as_str());
    query.bind(row.diagnostic_summary.as_deref());
    execute(runtime, client, query, "insert unresolved projection")
}

fn insert_event(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    event: &EventLogRow,
) -> Result<(), PersistenceError> {
    let mut query = Query::new(format!(
        "INSERT INTO [{schema}].event_log(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields)
         VALUES(@P1,CONVERT(DATETIME2(7),LEFT(@P2,27),127),CONVERT(DECIMAL(20,0),@P3),CONVERT(DATETIME2(7),LEFT(@P4,27),127),CONVERT(DECIMAL(20,0),@P5),@P6,@P7,@P8,@P9,@P10,CONVERT(DECIMAL(20,0),@P11),CONVERT(DECIMAL(20,0),@P12),CONVERT(DECIMAL(20,0),@P13),@P14,@P15,@P16,@P17,@P18,@P19,@P20)"
    ));
    bind_common(&mut query, event);
    query.bind(i64::from(event.event_type_id));
    query.bind(event.event_name.as_str());
    query.bind(event.has_unclassified_fields);
    execute(runtime, client, query, "insert event projection")
}

fn insert_value(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    schema: &str,
    value: LoggedValueRow,
) -> Result<(), PersistenceError> {
    let common = value.common;
    let mut query = Query::new(format!(
        "INSERT INTO [{schema}].logged_values(record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,value_id,value_name,value_type,unit,quality,semantic_role,boolean_value,signed_value,unsigned_value,number_value,text_value,exact_value,previous_boolean_value,previous_signed_value,previous_unsigned_value,previous_number_value,previous_text_value,previous_exact_value,is_audited,actor,reason,authorization_result)
         VALUES(@P1,CONVERT(DATETIME2(7),LEFT(@P2,27),127),CONVERT(DECIMAL(20,0),@P3),CONVERT(DATETIME2(7),LEFT(@P4,27),127),CONVERT(DECIMAL(20,0),@P5),@P6,@P7,@P8,@P9,@P10,CONVERT(DECIMAL(20,0),@P11),CONVERT(DECIMAL(20,0),@P12),CONVERT(DECIMAL(20,0),@P13),@P14,@P15,@P16,@P17,@P18,@P19,@P20,@P21,@P22,@P23,@P24,@P25,CONVERT(DECIMAL(20,0),@P26),@P27,@P28,@P29,@P30,@P31,CONVERT(DECIMAL(20,0),@P32),@P33,@P34,@P35,@P36,@P37,@P38,@P39)"
    ));
    bind_common(&mut query, &common);
    query.bind(i64::from(value.value_id));
    query.bind(value.value_name);
    query.bind(value.value_type);
    query.bind(value.unit);
    query.bind(value.quality.map(i32::from));
    query.bind(i32::from(value.semantic_role));
    query.bind(value.boolean_value);
    query.bind(value.signed_value);
    query.bind(value.unsigned_value);
    query.bind(value.number_value);
    query.bind(value.text_value);
    query.bind(value.exact_value);
    query.bind(value.previous_boolean_value);
    query.bind(value.previous_signed_value);
    query.bind(value.previous_unsigned_value);
    query.bind(value.previous_number_value);
    query.bind(value.previous_text_value);
    query.bind(value.previous_exact_value);
    query.bind(value.is_audited);
    query.bind(value.actor);
    query.bind(value.reason);
    query.bind(value.authorization_result);
    execute(runtime, client, query, "insert logged value projection")
}

fn bind_common<'a>(query: &mut Query<'a>, row: &'a EventLogRow) {
    query.bind(row.record_id.as_str());
    query.bind(row.event_time.as_deref());
    query.bind(row.event_time_ns.as_deref());
    query.bind(row.received_time.as_str());
    query.bind(row.received_time_ns.as_str());
    query.bind(row.source.as_deref());
    query.bind(i64::from(row.source_id));
    query.bind(row.source_path.as_str());
    query.bind(row.source_hierarchy.as_str());
    query.bind(i64::from(row.buffer_id));
    query.bind(row.run_id.as_str());
    query.bind(row.epoch_id.as_str());
    query.bind(row.sequence.as_str());
    query.bind(row.definition_hash.as_str());
    query.bind(row.time_unsynced);
    query.bind(row.synthetic_record);
    query.bind(row.partial_payload);
}

fn execute(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    query: Query<'_>,
    context: &str,
) -> Result<(), PersistenceError> {
    runtime
        .block_on(query.execute(client))
        .map(|_| ())
        .map_err(|error| super::sqlserver::sql_error(context, error))
}

fn batch(
    runtime: &tokio::runtime::Runtime,
    client: &mut TdsClient,
    sql: &str,
) -> Result<(), PersistenceError> {
    runtime
        .block_on(client.simple_query(sql))
        .map(|_| ())
        .map_err(|error| super::sqlserver::sql_error("create domain schema", error))
}

#[cfg(test)]
mod tests {
    #[test]
    fn logged_value_batches_stay_within_sql_server_parameter_limit() {
        let source = include_str!("sqlserver_read_model.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("SQL Server read-model production module");
        let batch = source
            .split("pub(super) fn insert_value_batch")
            .nth(1)
            .expect("logged-value batch function")
            .split("pub(super) fn insert_singleton_event_domains_batch")
            .next()
            .expect("logged-value batch body");

        assert!(production.contains("const SQLSERVER_LOGGED_VALUE_ROWS_PER_STATEMENT: usize = 53;"));
        assert!(batch.contains("values.chunks(SQLSERVER_LOGGED_VALUE_ROWS_PER_STATEMENT)"));
    }

    #[test]
    fn every_special_document_is_projected_without_first_match_collapse() {
        let source = include_str!("sqlserver_read_model.rs");
        let batch = source
            .split("pub(super) fn insert_loss_and_unresolved_batch")
            .nth(1)
            .expect("loss and unresolved batch function")
            .split("fn message_args")
            .next()
            .expect("loss and unresolved batch body");

        assert!(!batch.contains("find_map"));
        assert!(batch.contains("for domain in"));
    }

    #[test]
    fn loss_and_unresolved_inserts_bind_complete_provenance() {
        let source = include_str!("sqlserver_read_model.rs");
        let loss = source
            .split("\nfn insert_loss(")
            .nth(1)
            .expect("loss insert function")
            .split("fn insert_unresolved")
            .next()
            .expect("loss insert body");
        let unresolved = source
            .split("\nfn insert_unresolved(")
            .nth(1)
            .expect("unresolved insert function")
            .split("fn insert_event")
            .next()
            .expect("unresolved insert body");

        for field in [
            "row.source_path",
            "row.source_hierarchy",
            "row.time_unsynced",
            "row.synthetic_record",
            "row.partial_payload",
        ] {
            assert!(loss.contains(field), "missing loss binding: {field}");
            assert!(
                unresolved.contains(field),
                "missing unresolved binding: {field}"
            );
        }
    }
}
