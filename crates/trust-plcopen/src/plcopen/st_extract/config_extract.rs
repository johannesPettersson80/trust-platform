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
            invalid: false,
        };
        let mut reject_configuration = false;
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
                let mut resource_closed = false;
                while line_index < lines.len() {
                    let resource_line = lines[line_index].trim();
                    if resource_line.eq_ignore_ascii_case("END_RESOURCE") {
                        resource_closed = true;
                        break;
                    }
                    if let Some(task) = parse_task_declaration_line(resource_line) {
                        resource.tasks.push(task);
                    } else if let Some(program) = parse_program_binding_line(resource_line) {
                        resource.programs.push(program);
                    } else if resource_line
                        .to_ascii_uppercase()
                        .starts_with("TASK ")
                    {
                        configuration.invalid = true;
                    }
                    line_index += 1;
                }
                if !resource_closed {
                    reject_configuration = true;
                    warnings.push(format!(
                        "{}:{} RESOURCE '{}' missing END_RESOURCE",
                        source.path.display(),
                        line_index + 1,
                        resource.name
                    ));
                }
                configuration.resources.push(resource);
            } else if let Some(task) = parse_task_declaration_line(body_line) {
                configuration.tasks.push(task);
            } else if let Some(program) = parse_program_binding_line(body_line) {
                configuration.programs.push(program);
            } else if body_line.to_ascii_uppercase().starts_with("TASK ") {
                configuration.invalid = true;
                let task_name = body_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("<unnamed>")
                    .trim_end_matches('(')
                    .trim_end_matches(';');
                warnings.push(format!(
                    "{}:{} invalid TASK '{}'",
                    source.path.display(),
                    line_index + 1,
                    task_name
                ));
            }

            line_index += 1;
        }

        let task_names = configuration
            .tasks
            .iter()
            .map(|task| task.name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for program in &configuration.programs {
            if let Some(task_name) = &program.task_name {
                if !task_names.contains(&task_name.to_ascii_lowercase()) {
                    reject_configuration = true;
                    warnings.push(format!(
                        "{}:{} PROGRAM '{}' references unknown task '{}'",
                        source.path.display(),
                        line_index,
                        program.instance_name,
                        task_name
                    ));
                }
            }
        }
        for resource in &configuration.resources {
            let task_names = resource
                .tasks
                .iter()
                .map(|task| task.name.to_ascii_lowercase())
                .collect::<HashSet<_>>();
            for program in &resource.programs {
                if let Some(task_name) = &program.task_name {
                    if !task_names.contains(&task_name.to_ascii_lowercase()) {
                        reject_configuration = true;
                        warnings.push(format!(
                            "{}:{} PROGRAM '{}' references unknown task '{}'",
                            source.path.display(),
                            line_index,
                            program.instance_name,
                            task_name
                        ));
                    }
                }
            }
        }
        if !reject_configuration {
            declarations.push(configuration);
        }
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
                    return None;
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

    if task.single.is_some() && task.interval.is_some() {
        return None;
    }
    if task
        .interval
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
        || task
            .priority
            .as_ref()
            .is_some_and(|value| value.trim().parse::<u32>().is_err())
    {
        return None;
    }
    Some(task)
}

fn normalize_task_interval_literal(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("T#") || upper.starts_with("TIME#") || upper.starts_with("LTIME#") {
        let body = upper.split_once('#').map_or("", |(_, body)| body.trim());
        return if valid_iec_time_literal(body) {
            trimmed.to_string()
        } else {
            String::new()
        };
    }
    if upper.starts_with("PT") && upper.ends_with('S') {
        let number = &upper[2..upper.len() - 1];
        if let Ok(seconds) = number.parse::<f64>() {
            if !seconds.is_finite() || seconds < 0.0 {
                return String::new();
            }
            if seconds >= 1.0
                && (seconds.fract() - 0.0).abs() < f64::EPSILON
            {
                // Do not cast an out-of-range f64 to u64: Rust saturates that
                // conversion, which would turn an invalid ISO duration into
                // a plausible (but incorrect) IEC literal.
                let Ok(integer_seconds) = number.parse::<u64>() else {
                    return String::new();
                };
                return format!("T#{}s", integer_seconds);
            }
            let milliseconds = seconds * 1000.0;
            if !milliseconds.is_finite() || milliseconds > u64::MAX as f64 {
                return String::new();
            }
            return format!("T#{}ms", milliseconds.round() as u64);
        }
    }
    if upper.starts_with("PT") && upper.ends_with("MS") {
        let number = &upper[2..upper.len() - 2];
        if let Ok(millis) = number.parse::<i64>() {
            return if millis >= 0 {
                format!("T#{}ms", millis)
            } else {
                String::new()
            };
        }
    }
    String::new()
}

fn valid_iec_time_literal(body: &str) -> bool {
    if body.is_empty() {
        return false;
    }
    let mut index = 0usize;
    let mut components = 0usize;
    while index < body.len() {
        let start = index;
        while index < body.len()
            && body.as_bytes()[index].is_ascii_digit()
        {
            index += 1;
        }
        if index < body.len() && body.as_bytes()[index] == b'.' {
            index += 1;
            while index < body.len() && body.as_bytes()[index].is_ascii_digit() {
                index += 1;
            }
        }
        if start == index {
            return false;
        }
        let number = &body[start..index];
        if !number
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value >= 0.0)
        {
            return false;
        }
        let unit_start = index;
        while index < body.len() && body.as_bytes()[index].is_ascii_alphabetic() {
            index += 1;
        }
        let unit = &body[unit_start..index];
        if !matches!(unit, "MS" | "US" | "NS" | "S" | "M" | "H" | "D") {
            return false;
        }
        components += 1;
    }
    components > 0
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
