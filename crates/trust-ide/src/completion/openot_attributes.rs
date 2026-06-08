use trust_hir::openot_authoring as openot_vocab;

fn openot_attribute_completions(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Option<Vec<CompletionItem>> {
    let source = db.source_text(file_id);
    let offset = usize::try_from(u32::from(position)).ok()?;
    let context = OpenOtCompletionContext::at(source.as_str(), offset)?;
    let items = match context.mode {
        OpenOtCompletionMode::Kind => openot_kind_items(),
        OpenOtCompletionMode::Key(kind) => openot_key_items(kind),
        OpenOtCompletionMode::Value(key) => openot_value_items(key.as_str()),
    };
    (!items.is_empty()).then_some(items)
}

#[derive(Debug, Clone)]
struct OpenOtCompletionContext {
    mode: OpenOtCompletionMode,
}

#[derive(Debug, Clone)]
enum OpenOtCompletionMode {
    Kind,
    Key(openot_vocab::OotKind),
    Value(String),
}

impl OpenOtCompletionContext {
    fn at(source: &str, offset: usize) -> Option<Self> {
        if offset > source.len() || !source.is_char_boundary(offset) {
            return None;
        }
        let before_cursor = &source[..offset];
        let pragma_start = before_cursor.rfind("{attribute")?;
        if before_cursor[pragma_start..].contains('}') {
            return None;
        }

        let after_cursor = &source[offset..];
        let pragma_end = after_cursor
            .find('}')
            .map_or(source.len(), |relative| offset + relative);
        let pragma_text = &source[pragma_start..pragma_end.min(source.len())];
        if !pragma_text.trim_start().starts_with("{attribute") {
            return None;
        }

        let segment = before_cursor[pragma_start..]
            .rsplit(',')
            .next()
            .unwrap_or_default();
        if let Some((before_assignment, _)) = segment.split_once(":=") {
            let key = quoted_tokens(before_assignment).into_iter().last()?;
            if key.eq_ignore_ascii_case("oot") {
                return Some(Self {
                    mode: OpenOtCompletionMode::Kind,
                });
            }
            return Some(Self {
                mode: OpenOtCompletionMode::Value(key.to_ascii_lowercase()),
            });
        }

        let parse_text;
        let parse_input = if pragma_text.contains('}') {
            pragma_text
        } else {
            parse_text = format!("{pragma_text}}}");
            parse_text.as_str()
        };
        let attrs = openot_vocab::parse_attribute_entries_from_text(
            parse_input,
            TextRange::empty(TextSize::from(0)),
        );
        let kind = attrs
            .iter()
            .find(|entry| entry.key == "oot")
            .and_then(|entry| openot_vocab::OotKind::parse(&entry.value));
        if let Some(kind) = kind {
            Some(Self {
                mode: OpenOtCompletionMode::Key(kind),
            })
        } else {
            Some(Self {
                mode: OpenOtCompletionMode::Kind,
            })
        }
    }
}

fn openot_kind_items() -> Vec<CompletionItem> {
    openot_vocab::KINDS
        .iter()
        .map(|kind| {
            CompletionItem::new(*kind, CompletionKind::Snippet)
                .with_detail("OpenOT kind")
                .with_insert_text(format!("'{kind}'"))
                .with_priority(1)
        })
        .collect()
}

fn openot_key_items(kind: openot_vocab::OotKind) -> Vec<CompletionItem> {
    openot_vocab::allowed_keys(kind)
        .iter()
        .map(|key| {
            let insert = match *key {
                "category" => "'category' := '${1:process}'",
                "model" => "'model' := '${1:ISA-88}'",
                "unit" => "'unit' := '${1:L}'",
                "deadband" => "'deadband' := '${1:0.5}'",
                "class" => "'class' := '${1:alarm}'",
                "severity" => "'severity' := '${1:900}'",
                "template" => "'template' := '${1:message}'",
                "cause" => "'cause' := '${1:VariableName}'",
                "arg1" => "'arg1' := '${1:VariableName}'",
                "arg2" => "'arg2' := '${1:VariableName}'",
                "arg3" => "'arg3' := '${1:VariableName}'",
                "arg4" => "'arg4' := '${1:VariableName}'",
                "quality" => "'quality' := '${1:good}'",
                "semanticrole" => "'semanticRole' := '${1:actual}'",
                "previous" => "'previous' := '${1:true}'",
                "sampling" => "'sampling' := '${1:on-change}'",
                "interval" => "'interval' := '${1:1000}'",
                "of" => "'of' := ${1:AlarmVariable}",
                "event" => "'event' := '${1:acknowledge}'",
                "by" => "'by' := ${1:OperatorName}",
                "reason" => "'reason' := ${1:ReasonText}",
                _ => *key,
            };
            CompletionItem::new(*key, CompletionKind::Snippet)
                .with_detail(format!("OpenOT {} key", kind.as_str()))
                .with_insert_text(insert)
                .with_priority(1)
        })
        .collect()
}

fn openot_value_items(key: &str) -> Vec<CompletionItem> {
    match key {
        "oot" => openot_kind_items(),
        "category" => string_value_items("OpenOT state category", openot_vocab::CATEGORY_VALUES),
        "model" => string_value_items("OpenOT procedural model", openot_vocab::MODEL_VALUES),
        "class" => string_value_items("OpenOT condition class", openot_vocab::CLASS_VALUES),
        "unit" => string_value_items("OpenOT engineering unit", openot_vocab::UNIT_VALUES),
        "quality" => string_value_items("OpenOT value quality", openot_vocab::QUALITY_VALUES),
        "semanticrole" => string_value_items(
            "OpenOT value semantic role",
            openot_vocab::SEMANTIC_ROLE_VALUES,
        ),
        "previous" => string_value_items("OpenOT previous-value capture", &["true", "false"]),
        "sampling" => {
            string_value_items("OpenOT value sampling policy", openot_vocab::SAMPLING_VALUES)
        }
        "event" => string_value_items(
            "OpenOT condition lifecycle event",
            openot_vocab::CONDITION_EVENT_VALUES,
        ),
        "interval" => vec![CompletionItem::new("1000", CompletionKind::Snippet)
            .with_detail("OpenOT periodic interval")
            .with_documentation("Interval is a positive integer number of milliseconds.")
            .with_insert_text("'${1:1000}'")
            .with_priority(1)],
        "severity" => vec![CompletionItem::new("900", CompletionKind::Snippet)
            .with_detail("OpenOT severity")
            .with_documentation("Severity is 1..1000: low 1..332, medium 333..666, high 667..1000.")
            .with_insert_text("'${1:900}'")
            .with_priority(1)],
        _ => Vec::new(),
    }
}

fn string_value_items(detail: &str, values: &[&str]) -> Vec<CompletionItem> {
    values
        .iter()
        .map(|value| {
            CompletionItem::new(*value, CompletionKind::Snippet)
                .with_detail(detail)
                .with_insert_text(format!("'{value}'"))
                .with_priority(1)
        })
        .collect()
}

fn quoted_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('\'') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('\'') else {
            break;
        };
        tokens.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }
    tokens
}
