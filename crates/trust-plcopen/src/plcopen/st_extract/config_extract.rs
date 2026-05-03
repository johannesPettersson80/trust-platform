fn extract_configuration_declarations(
    source: &LoadedSource,
) -> (Vec<ConfigurationDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();
    let lines = source.text.lines().collect::<Vec<_>>();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line = lines[line_index];
        if !line
            .trim_start()
            .to_ascii_uppercase()
            .starts_with("CONFIGURATION ")
        {
            line_index += 1;
            continue;
        }

        let Some(name) = line
            .split_whitespace()
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        else {
            warnings.push(format!(
                "{}:{} CONFIGURATION declaration without name skipped",
                source.path.display(),
                line_index + 1
            ));
            line_index += 1;
            continue;
        };

        let mut configuration = ConfigurationDecl {
            name,
            tasks: Vec::new(),
            programs: Vec::new(),
            resources: Vec::new(),
        };
        line_index += 1;

        while line_index < lines.len() {
            let body_line = lines[line_index].trim();
            if body_line.eq_ignore_ascii_case("END_CONFIGURATION") {
                break;
            }

            if body_line.to_ascii_uppercase().starts_with("RESOURCE ") {
                let (resource_name, target) =
                    parse_resource_header(body_line).unwrap_or_else(|| {
                        (
                            format!("Resource{}", configuration.resources.len() + 1),
                            "CPU".to_string(),
                        )
                    });
                let mut resource = ResourceDecl {
                    name: resource_name,
                    target,
                    tasks: Vec::new(),
                    programs: Vec::new(),
                };
                line_index += 1;
                while line_index < lines.len() {
                    let resource_line = lines[line_index].trim();
                    if resource_line.eq_ignore_ascii_case("END_RESOURCE") {
                        break;
                    }
                    if let Some(task) = parse_task_declaration_line(resource_line) {
                        resource.tasks.push(task);
                    } else if let Some(program) = parse_program_binding_line(resource_line) {
                        resource.programs.push(program);
                    }
                    line_index += 1;
                }
                configuration.resources.push(resource);
            } else if let Some(task) = parse_task_declaration_line(body_line) {
                configuration.tasks.push(task);
            } else if let Some(program) = parse_program_binding_line(body_line) {
                configuration.programs.push(program);
            }

            line_index += 1;
        }

        declarations.push(configuration);
        line_index += 1;
    }

    (declarations, warnings)
}

fn parse_resource_header(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim().trim_end_matches(';');
    let mut parts = trimmed.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("RESOURCE") {
        return None;
    }
    let name = parts.next()?.to_string();
    let mut target = "CPU".to_string();
    while let Some(token) = parts.next() {
        if token.eq_ignore_ascii_case("ON") {
            if let Some(value) = parts.next() {
                target = value.to_string();
            }
            break;
        }
    }
    Some((name, target))
}

fn parse_task_declaration_line(line: &str) -> Option<TaskDecl> {
    let trimmed = line.trim();
    if !trimmed.to_ascii_uppercase().starts_with("TASK ") {
        return None;
    }
    let no_suffix = trimmed.trim_end_matches(';');
    let rest = no_suffix.get(4..)?.trim();
    let task_name_end = rest
        .find(|ch: char| ch.is_whitespace() || ch == '(')
        .unwrap_or(rest.len());
    let name = rest[..task_name_end].trim();
    if name.is_empty() {
        return None;
    }

    let mut task = TaskDecl {
        name: name.to_string(),
        ..TaskDecl::default()
    };

    if let (Some(open), Some(close)) = (rest.find('('), rest.rfind(')')) {
        if close > open {
            let init = &rest[open + 1..close];
            for item in init.split(',') {
                let Some((key, value)) = item.split_once(":=") else {
                    continue;
                };
                let key = key.trim().to_ascii_uppercase();
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match key.as_str() {
                    "INTERVAL" => task.interval = Some(normalize_task_interval_literal(value)),
                    "SINGLE" => task.single = Some(value.to_string()),
                    "PRIORITY" => task.priority = Some(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    Some(task)
}

fn normalize_task_interval_literal(value: &str) -> String {
    let trimmed = value.trim();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("T#") || upper.starts_with("TIME#") || upper.starts_with("LTIME#") {
        return trimmed.to_string();
    }
    if upper.starts_with("PT") && upper.ends_with('S') {
        let number = &upper[2..upper.len() - 1];
        if let Ok(seconds) = number.parse::<f64>() {
            if seconds >= 1.0 && (seconds.fract() - 0.0).abs() < f64::EPSILON {
                return format!("T#{}s", seconds as u64);
            }
            return format!("T#{}ms", (seconds * 1000.0).round() as i64);
        }
    }
    if upper.starts_with("PT") && upper.ends_with("MS") {
        let number = &upper[2..upper.len() - 2];
        if let Ok(millis) = number.parse::<i64>() {
            return format!("T#{}ms", millis);
        }
    }
    trimmed.to_string()
}

fn parse_program_binding_line(line: &str) -> Option<ProgramBindingDecl> {
    let trimmed = line.trim();
    if !trimmed.to_ascii_uppercase().starts_with("PROGRAM ") {
        return None;
    }
    let mut rest = trimmed.trim_end_matches(';').get(7..)?.trim();
    if rest.to_ascii_uppercase().starts_with("RETAIN ") {
        rest = rest.get(7..)?.trim();
    } else if rest.to_ascii_uppercase().starts_with("NON_RETAIN ") {
        rest = rest.get(11..)?.trim();
    }
    let (lhs, rhs) = rest.split_once(':')?;
    let mut lhs_parts = lhs.split_whitespace();
    let instance_name = lhs_parts.next()?.trim().to_string();
    if instance_name.is_empty() {
        return None;
    }

    let mut task_name = None;
    while let Some(token) = lhs_parts.next() {
        if token.eq_ignore_ascii_case("WITH") {
            task_name = lhs_parts.next().map(ToOwned::to_owned);
            break;
        }
    }

    let rhs = rhs.trim();
    let type_name = rhs
        .split_once('(')
        .map_or(rhs, |(head, _)| head)
        .trim()
        .trim_end_matches(';')
        .to_string();
    if type_name.is_empty() {
        return None;
    }

    Some(ProgramBindingDecl {
        instance_name,
        task_name,
        type_name,
    })
}
