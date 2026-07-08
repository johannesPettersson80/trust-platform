pub fn build_values(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    read_only: bool,
    ids: Option<&[String]>,
) -> HmiValuesResult {
    let requested = ids.map(|entries| entries.iter().map(String::as_str).collect::<HashSet<_>>());
    let points = collect_points(resource_name, metadata, snapshot, read_only);
    let now_ms = now_unix_ms();
    let mut values = IndexMap::new();

    for point in points {
        if let Some(requested) = requested.as_ref() {
            if !requested.contains(point.id.as_str()) {
                continue;
            }
        }
        let (value, quality) = if let Some(snapshot) = snapshot {
            match resolve_point_value(&point.binding, snapshot) {
                Some(value) => (value_to_json(value), "good"),
                None => (serde_json::Value::Null, "bad"),
            }
        } else {
            (serde_json::Value::Null, "stale")
        };
        values.insert(
            point.id,
            HmiValueRecord {
                v: value,
                q: quality,
                ts_ms: now_ms,
            },
        );
    }

    HmiValuesResult {
        connected: snapshot.is_some(),
        timestamp_ms: now_ms,
        source_time_ns: snapshot.map(|state| state.now.as_nanos()),
        freshness_ms: snapshot.map(|_| 0),
        values,
    }
}

pub fn resolve_write_point(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    target: &str,
) -> Option<HmiWritePoint> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    collect_points(resource_name, metadata, snapshot, true)
        .into_iter()
        .find(|point| point.id == target || point.path == target)
        .map(|point| HmiWritePoint {
            id: point.id,
            path: point.path,
            data_type: point.data_type,
            binding: match point.binding {
                HmiBinding::ProgramVar { program, variable } => {
                    HmiWriteBinding::ProgramVar { program, variable }
                }
                HmiBinding::Global { name } => HmiWriteBinding::Global { name },
            },
        })
}

pub fn resolve_write_value_template(
    point: &HmiWritePoint,
    snapshot: &DebugSnapshot,
) -> Option<Value> {
    match &point.binding {
        HmiWriteBinding::ProgramVar { program, variable } => {
            let Value::Instance(instance_id) = snapshot.storage.get_global(program.as_str())?
            else {
                return None;
            };
            snapshot
                .storage
                .get_instance(*instance_id)
                .and_then(|instance| instance.variables.get(variable.as_str()))
                .cloned()
        }
        HmiWriteBinding::Global { name } => snapshot.storage.get_global(name.as_str()).cloned(),
    }
}
