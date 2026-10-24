fn normalize_scaffold_style(style: &str) -> String {
    let candidate = style.trim().to_ascii_lowercase();
    if theme_palette(candidate.as_str()).is_some() {
        candidate
    } else {
        "control-room".to_string()
    }
}

pub(super) fn collect_scaffold_points(
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    source_index: &SourceSymbolIndex,
) -> Vec<ScaffoldPoint> {
    // A compiled program's points come from its symbol table (specification 44 §13.3).
    if let Some(state) = snapshot.and_then(|snapshot| snapshot.compiled.as_deref()) {
        let mut points = compiled_scaffold_points(state, source_index);
        sort_scaffold_points(&mut points);
        return points;
    }
    let mut points = Vec::new();

    for (program_name, program) in metadata.programs() {
        let program_key = program_name.to_ascii_uppercase();
        let program_has_entries = source_index.programs_with_entries.contains(&program_key);
        let mut program_points = Vec::new();
        let mut external_points_added = false;
        for variable in &program.vars {
            if variable.constant {
                continue;
            }
            let key = normalize_symbol_key(program_name.as_str(), variable.name.as_str());
            let qualifier = source_index
                .program_vars
                .get(key.as_str())
                .copied()
                .unwrap_or(if program_has_entries {
                    SourceVarKind::Unknown
                } else {
                    SourceVarKind::Output
                });
            if program_has_entries && !qualifier.is_external() {
                continue;
            }
            external_points_added = true;

            let writable = qualifier.is_writable();
            let ty = metadata.registry().get(variable.type_id);
            let data_type = metadata
                .registry()
                .type_name(variable.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let type_bucket = ty
                .map(scaffold_type_bucket_for_type)
                .unwrap_or(ScaffoldTypeBucket::Other);
            let widget = ty
                .map(|ty| widget_for_scaffold_type(ty, writable, qualifier).to_string())
                .unwrap_or_else(|| "value".to_string());
            let path = format!("{program_name}.{}", variable.name);
            let (unit, min, max) =
                infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

            program_points.push(ScaffoldPoint {
                program: program_name.to_string(),
                raw_name: variable.name.to_string(),
                path,
                label: infer_label(variable.name.as_str()),
                data_type: data_type.clone(),
                widget,
                writable,
                qualifier,
                inferred_interface: !program_has_entries,
                type_bucket,
                unit,
                min,
                max,
                enum_values: ty.map(enum_values_for_type).unwrap_or_default(),
            });
        }

        if program_has_entries && !external_points_added {
            for variable in &program.vars {
                if variable.constant {
                    continue;
                }
                let ty = metadata.registry().get(variable.type_id);
                let data_type = metadata
                    .registry()
                    .type_name(variable.type_id)
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                let type_bucket = ty
                    .map(scaffold_type_bucket_for_type)
                    .unwrap_or(ScaffoldTypeBucket::Other);
                let widget = ty
                    .map(|ty| {
                        widget_for_scaffold_type(ty, false, SourceVarKind::Output).to_string()
                    })
                    .unwrap_or_else(|| "value".to_string());
                let path = format!("{program_name}.{}", variable.name);
                let (unit, min, max) =
                    infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

                program_points.push(ScaffoldPoint {
                    program: program_name.to_string(),
                    raw_name: variable.name.to_string(),
                    path,
                    label: infer_label(variable.name.as_str()),
                    data_type: data_type.clone(),
                    widget,
                    writable: false,
                    qualifier: SourceVarKind::Unknown,
                    inferred_interface: true,
                    type_bucket,
                    unit,
                    min,
                    max,
                    enum_values: ty.map(enum_values_for_type).unwrap_or_default(),
                });
            }
        }

        points.extend(program_points);
    }

    if let Some(snapshot) = snapshot {
        let program_names = metadata
            .programs()
            .keys()
            .map(|name| name.to_ascii_uppercase())
            .collect::<HashSet<_>>();
        let has_global_filter = !source_index.globals.is_empty();
        for (name, value) in snapshot.storage.globals() {
            if program_names.contains(&name.to_ascii_uppercase()) {
                continue;
            }
            if matches!(value, Value::Instance(_)) {
                continue;
            }
            if has_global_filter && !source_index.globals.contains(&name.to_ascii_uppercase()) {
                continue;
            }

            let data_type = value_type_name(value).unwrap_or_else(|| "UNKNOWN".to_string());
            let type_bucket = scaffold_type_bucket_for_value(value, data_type.as_str());
            let path = format!("global.{name}");
            let (unit, min, max) =
                infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

            points.push(ScaffoldPoint {
                program: "global".to_string(),
                raw_name: name.to_string(),
                path,
                label: infer_label(name.as_str()),
                data_type,
                widget: widget_for_scaffold_value(value).to_string(),
                writable: false,
                qualifier: SourceVarKind::Global,
                inferred_interface: !has_global_filter,
                type_bucket,
                unit,
                min,
                max,
                enum_values: Vec::new(),
            });
        }
    }

    sort_scaffold_points(&mut points);
    points
}

fn sort_scaffold_points(points: &mut [ScaffoldPoint]) {
    points.sort_by(|left, right| {
        left.program
            .cmp(&right.program)
            .then_with(|| left.path.cmp(&right.path))
    });
}

/// The scaffold points of the compiled program whose state is `state`, chosen as an interpreted
/// program's are: each program instance's interface variables, which the sources name, or all its
/// variables read-only when the sources name none of them; and the globals the sources declare.
fn compiled_scaffold_points(
    state: &CompiledState,
    source_index: &SourceSymbolIndex,
) -> Vec<ScaffoldPoint> {
    let variables = state.variables();
    let mut points = Vec::new();
    for program_name in state.programs() {
        let program_key = program_name.to_ascii_uppercase();
        let program_has_entries = source_index.programs_with_entries.contains(&program_key);
        let program_vars = variables
            .iter()
            .filter(|variable| {
                !variable.constant && variable.program.as_ref() == Some(&program_name)
            })
            .collect::<Vec<_>>();
        let mut program_points = Vec::new();
        for variable in &program_vars {
            let key = normalize_symbol_key(program_name.as_str(), variable.member.as_str());
            let qualifier = source_index
                .program_vars
                .get(key.as_str())
                .copied()
                .unwrap_or(if program_has_entries {
                    SourceVarKind::Unknown
                } else {
                    SourceVarKind::Output
                });
            if program_has_entries && !qualifier.is_external() {
                continue;
            }
            program_points.push(compiled_scaffold_point(
                state,
                variable,
                program_name.as_str(),
                qualifier.is_writable(),
                qualifier,
                !program_has_entries,
            ));
        }
        if program_has_entries && program_points.is_empty() {
            for variable in &program_vars {
                let mut point = compiled_scaffold_point(
                    state,
                    variable,
                    program_name.as_str(),
                    false,
                    SourceVarKind::Output,
                    true,
                );
                point.qualifier = SourceVarKind::Unknown;
                program_points.push(point);
            }
        }
        points.extend(program_points);
    }

    let has_global_filter = !source_index.globals.is_empty();
    for variable in &variables {
        if variable.program.is_some() || variable.instance {
            continue;
        }
        if has_global_filter
            && !source_index
                .globals
                .contains(&variable.name.to_ascii_uppercase())
        {
            continue;
        }
        let mut point = compiled_scaffold_point(
            state,
            variable,
            "global",
            false,
            SourceVarKind::Global,
            !has_global_filter,
        );
        point.path = format!("global.{}", variable.name);
        let (unit, min, max) = infer_unit_and_range(
            point.path.as_str(),
            point.data_type.as_str(),
            point.type_bucket,
        );
        (point.unit, point.min, point.max) = (unit, min, max);
        point.enum_values = Vec::new();
        points.push(point);
    }
    points
}

/// The scaffold point of the compiled program's `variable`, of `program`, its widget, type
/// bucket, unit and range following its type and its value as an interpreted program's follow
/// the type.
fn compiled_scaffold_point(
    state: &CompiledState,
    variable: &CompiledVariable,
    program: &str,
    writable: bool,
    qualifier: SourceVarKind,
    inferred_interface: bool,
) -> ScaffoldPoint {
    let value = state.value_of(variable.symbol);
    let data_type = if variable.type_name.is_empty() {
        "UNKNOWN".to_string()
    } else {
        variable.type_name.to_string()
    };
    let (widget, type_bucket) = if variable.instance {
        ("tree", ScaffoldTypeBucket::Composite)
    } else {
        match &value {
            Some(Value::Real(_) | Value::LReal(_))
                if !writable
                    && matches!(qualifier, SourceVarKind::Output | SourceVarKind::Global) =>
            {
                ("gauge", ScaffoldTypeBucket::Numeric)
            }
            Some(value) => (
                widget_for_value(value, writable),
                scaffold_type_bucket_for_value(value, data_type.as_str()),
            ),
            None => (
                "value",
                if is_numeric_data_type(data_type.as_str()) {
                    ScaffoldTypeBucket::Numeric
                } else {
                    ScaffoldTypeBucket::Other
                },
            ),
        }
    };
    let path = format!("{program}.{}", variable.member);
    let (unit, min, max) = infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);
    ScaffoldPoint {
        program: program.to_string(),
        raw_name: variable.member.to_string(),
        path,
        label: infer_label(variable.member.as_str()),
        data_type,
        widget: widget.to_string(),
        writable,
        qualifier,
        inferred_interface,
        type_bucket,
        unit,
        min,
        max,
        enum_values: state
            .enum_values(variable.symbol)
            .into_iter()
            .map(|name| name.to_string())
            .collect(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ScaffoldOverviewCategory {
    SafetyAlarm,
    CommandMode,
    Kpi,
    Deviation,
    Inventory,
    Diagnostic,
}

impl ScaffoldOverviewCategory {
    const fn weight(self) -> i32 {
        match self {
            Self::SafetyAlarm => 100,
            Self::CommandMode => 80,
            Self::Kpi => 60,
            Self::Deviation => 45,
            Self::Inventory => 35,
            Self::Diagnostic => 20,
        }
    }

    const fn slot_cap(self) -> usize {
        match self {
            Self::SafetyAlarm => 2,
            Self::CommandMode => 2,
            Self::Kpi | Self::Deviation => 4,
            Self::Inventory => 2,
            Self::Diagnostic => 2,
        }
    }
}

fn classify_overview_category(point: &ScaffoldPoint) -> ScaffoldOverviewCategory {
    let name = format!(
        "{} {} {}",
        point.path.to_ascii_lowercase(),
        point.raw_name.to_ascii_lowercase(),
        point.label.to_ascii_lowercase()
    );
    if contains_any(
        name.as_str(),
        &[
            "alarm",
            "fault",
            "trip",
            "interlock",
            "estop",
            "emergency",
            "safety",
        ],
    ) {
        return ScaffoldOverviewCategory::SafetyAlarm;
    }
    if contains_any(name.as_str(), &["deviation", "delta", "error", "diff"]) {
        return ScaffoldOverviewCategory::Deviation;
    }
    if contains_any(
        name.as_str(),
        &[
            "inventory",
            "tank",
            "feed",
            "source",
            "product",
            "stock",
            "level",
        ],
    ) {
        return ScaffoldOverviewCategory::Inventory;
    }
    if point.writable
        || contains_any(
            name.as_str(),
            &[
                "mode", "cmd", "command", "start", "stop", "reset", "enable", "bypass",
            ],
        )
    {
        return ScaffoldOverviewCategory::CommandMode;
    }
    if point.type_bucket == ScaffoldTypeBucket::Numeric
        && contains_any(
            name.as_str(),
            &[
                "flow",
                "pressure",
                "temp",
                "temperature",
                "speed",
                "rpm",
                "current",
                "voltage",
                "power",
                "rate",
                "level",
            ],
        )
    {
        return ScaffoldOverviewCategory::Kpi;
    }
    ScaffoldOverviewCategory::Diagnostic
}

fn select_scaffold_overview_points(points: Vec<ScaffoldPoint>) -> Vec<ScaffoldPoint> {
    let budget = 10_usize;
    if points.len() <= budget {
        return points;
    }

    let mut scored = points
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            let category = classify_overview_category(&point);
            (category.weight(), category, index, point)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.path.cmp(&right.3.path))
    });

    let mut selected = Vec::new();
    let mut overflow = Vec::new();
    let mut category_counts = HashMap::<ScaffoldOverviewCategory, usize>::new();

    for item in scored {
        let count = category_counts.get(&item.1).copied().unwrap_or_default();
        if count < item.1.slot_cap() && selected.len() < budget {
            category_counts.insert(item.1, count + 1);
            selected.push(item);
        } else {
            overflow.push(item);
        }
    }
    for item in overflow {
        if selected.len() >= budget {
            break;
        }
        selected.push(item);
    }

    selected
        .into_iter()
        .map(|(_, _, _, point)| point)
        .collect::<Vec<_>>()
}

