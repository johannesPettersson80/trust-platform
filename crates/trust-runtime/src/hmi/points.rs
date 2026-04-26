fn collect_points(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    read_only: bool,
) -> Vec<HmiPoint> {
    let resource = stable_component(resource_name);
    let writable = !read_only;
    let mut points = Vec::new();

    for (program_name, program) in metadata.programs() {
        for variable in &program.vars {
            let ty = metadata.registry().get(variable.type_id);
            let data_type = metadata
                .registry()
                .type_name(variable.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let widget = ty
                .map(|ty| widget_for_type(ty, writable).to_string())
                .unwrap_or_else(|| "value".to_string());
            let path = format!("{program_name}.{}", variable.name);
            points.push(HmiPoint {
                id: format!(
                    "resource/{resource}/program/{}/field/{}",
                    stable_component(program_name.as_str()),
                    stable_component(variable.name.as_str())
                ),
                path,
                label: variable.name.to_string(),
                data_type,
                access: if writable { "read_write" } else { "read" },
                writable,
                widget,
                source: format!("program:{program_name}"),
                page: DEFAULT_PAGE_ID.to_string(),
                group: DEFAULT_GROUP_NAME.to_string(),
                order: 0,
                zones: Vec::new(),
                on_color: None,
                off_color: None,
                section_title: None,
                widget_span: None,
                alarm_deadband: None,
                inferred_interface: false,
                detail_page: None,
                unit: None,
                min: None,
                max: None,
                binding: HmiBinding::ProgramVar {
                    program: program_name.clone(),
                    variable: variable.name.clone(),
                },
            });
        }
    }

    if let Some(snapshot) = snapshot {
        let programs = metadata
            .programs()
            .keys()
            .map(|name| name.to_ascii_uppercase())
            .collect::<HashSet<_>>();
        for (name, value) in snapshot.storage.globals() {
            if programs.contains(&name.to_ascii_uppercase()) {
                continue;
            }
            if matches!(value, Value::Instance(_)) {
                continue;
            }
            let data_type = value_type_name(value).unwrap_or_else(|| "UNKNOWN".to_string());
            points.push(HmiPoint {
                id: format!(
                    "resource/{resource}/global/{}",
                    stable_component(name.as_str())
                ),
                path: format!("global.{name}"),
                label: name.to_string(),
                data_type,
                access: if writable { "read_write" } else { "read" },
                writable,
                widget: widget_for_value(value, writable).to_string(),
                source: "global".to_string(),
                page: DEFAULT_PAGE_ID.to_string(),
                group: DEFAULT_GROUP_NAME.to_string(),
                order: 0,
                zones: Vec::new(),
                on_color: None,
                off_color: None,
                section_title: None,
                widget_span: None,
                alarm_deadband: None,
                inferred_interface: false,
                detail_page: None,
                unit: None,
                min: None,
                max: None,
                binding: HmiBinding::Global { name: name.clone() },
            });
        }
    }

    points
}

fn resolve_point_value<'a>(binding: &HmiBinding, snapshot: &'a DebugSnapshot) -> Option<&'a Value> {
    match binding {
        HmiBinding::ProgramVar { program, variable } => {
            let Value::Instance(instance_id) = snapshot.storage.get_global(program.as_str())?
            else {
                return None;
            };
            snapshot
                .storage
                .get_instance(*instance_id)
                .and_then(|instance| instance.variables.get(variable.as_str()))
        }
        HmiBinding::Global { name } => snapshot.storage.get_global(name.as_str()),
    }
}

fn widget_for_type(ty: &Type, writable: bool) -> &'static str {
    match ty {
        Type::Bool => {
            if writable {
                "toggle"
            } else {
                "indicator"
            }
        }
        Type::Enum { .. } => {
            if writable {
                "selector"
            } else {
                "readout"
            }
        }
        Type::Array { .. } => "table",
        Type::Struct { .. }
        | Type::Union { .. }
        | Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Interface { .. } => "tree",
        ty if ty.is_string() || ty.is_char() => "text",
        ty if (ty.is_numeric() || ty.is_bit_string() || ty.is_time()) && writable => "slider",
        ty if ty.is_numeric() || ty.is_bit_string() || ty.is_time() => "value",
        _ => "value",
    }
}

fn widget_for_value(value: &Value, writable: bool) -> &'static str {
    match value {
        Value::Bool(_) => {
            if writable {
                "toggle"
            } else {
                "indicator"
            }
        }
        Value::Enum(_) => {
            if writable {
                "selector"
            } else {
                "readout"
            }
        }
        Value::Array(_) => "table",
        Value::Struct(_) | Value::Instance(_) => "tree",
        Value::String(_) | Value::WString(_) | Value::Char(_) | Value::WChar(_) => "text",
        Value::SInt(_)
        | Value::Int(_)
        | Value::DInt(_)
        | Value::LInt(_)
        | Value::USInt(_)
        | Value::UInt(_)
        | Value::UDInt(_)
        | Value::ULInt(_)
        | Value::Real(_)
        | Value::LReal(_)
        | Value::Byte(_)
        | Value::Word(_)
        | Value::DWord(_)
        | Value::LWord(_)
        | Value::Time(_)
        | Value::LTime(_)
        | Value::Date(_)
        | Value::LDate(_)
        | Value::Tod(_)
        | Value::LTod(_)
        | Value::Dt(_)
        | Value::Ldt(_) => {
            if writable {
                "slider"
            } else {
                "value"
            }
        }
        Value::Reference(_) | Value::Null => "value",
    }
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::SInt(value) => serde_json::json!(*value),
        Value::Int(value) => serde_json::json!(*value),
        Value::DInt(value) => serde_json::json!(*value),
        Value::LInt(value) => serde_json::json!(*value),
        Value::USInt(value) => serde_json::json!(*value),
        Value::UInt(value) => serde_json::json!(*value),
        Value::UDInt(value) => serde_json::json!(*value),
        Value::ULInt(value) => serde_json::json!(*value),
        Value::Real(value) => serde_json::json!(*value),
        Value::LReal(value) => serde_json::json!(*value),
        Value::Byte(value) => serde_json::json!(*value),
        Value::Word(value) => serde_json::json!(*value),
        Value::DWord(value) => serde_json::json!(*value),
        Value::LWord(value) => serde_json::json!(*value),
        Value::Time(value) | Value::LTime(value) => serde_json::json!(value.as_nanos()),
        Value::Date(value) => serde_json::json!(value.ticks()),
        Value::LDate(value) => serde_json::json!(value.nanos()),
        Value::Tod(value) => serde_json::json!(value.ticks()),
        Value::LTod(value) => serde_json::json!(value.nanos()),
        Value::Dt(value) => serde_json::json!(value.ticks()),
        Value::Ldt(value) => serde_json::json!(value.nanos()),
        Value::String(value) => serde_json::json!(value.as_str()),
        Value::WString(value) => serde_json::json!(value),
        Value::Char(value) => {
            let text = char::from_u32((*value).into()).unwrap_or('?').to_string();
            serde_json::json!(text)
        }
        Value::WChar(value) => {
            let text = char::from_u32((*value).into()).unwrap_or('?').to_string();
            serde_json::json!(text)
        }
        Value::Array(value) => {
            serde_json::Value::Array(value.elements().iter().map(value_to_json).collect())
        }
        Value::Struct(value) => {
            let mut object = serde_json::Map::new();
            for (name, field) in value.fields() {
                object.insert(name.to_string(), value_to_json(field));
            }
            serde_json::Value::Object(object)
        }
        Value::Enum(value) => serde_json::json!({
            "type": value.type_name().as_str(),
            "variant": value.variant_name().as_str(),
            "value": value.numeric_value(),
        }),
        Value::Reference(_) => serde_json::Value::Null,
        Value::Instance(value) => serde_json::json!({ "instance": value.0 }),
        Value::Null => serde_json::Value::Null,
    }
}

fn stable_component(value: &str) -> String {
    let text = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if text.is_empty() {
        "unnamed".to_string()
    } else {
        text
    }
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
