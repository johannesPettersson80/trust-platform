fn config_type_error(key: &str, expected: &str) -> String {
    format!("invalid config value for '{key}': expected {expected}")
}

fn config_value_error(key: &str, message: &str) -> String {
    format!("invalid config value for '{key}': {message}")
}

fn expect_bool(key: &str, value: &serde_json::Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| config_type_error(key, "boolean"))
}

fn expect_non_empty_string<'a>(key: &str, value: &'a serde_json::Value) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| config_type_error(key, "string"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(config_value_error(key, "must not be empty"));
    }
    Ok(value)
}

fn expect_positive_i64(key: &str, value: &serde_json::Value) -> Result<i64, String> {
    let number = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .ok_or_else(|| config_type_error(key, "integer >= 1"))?;
    if number < 1 {
        return Err(config_value_error(key, "must be >= 1"));
    }
    Ok(number)
}

fn expect_string_array(key: &str, value: &serde_json::Value) -> Result<Vec<String>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| config_type_error(key, "array of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let Some(text) = item.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry {index} must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry {index} must not be empty"),
            ));
        }
        output.push(text.to_string());
    }
    Ok(output)
}

fn expect_string_map(
    key: &str,
    value: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let values = value
        .as_object()
        .ok_or_else(|| config_type_error(key, "object of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (map_key, map_value) in values {
        if map_key.trim().is_empty() {
            return Err(config_value_error(key, "map keys must not be empty"));
        }
        let Some(text) = map_value.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must not be empty"),
            ));
        }
        output.push((map_key.trim().to_string(), text.to_string()));
    }
    output.sort();
    Ok(output)
}

fn expect_wan_allow_write_rules(
    key: &str,
    value: &serde_json::Value,
) -> Result<Vec<RuntimeCloudWanAllowRule>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| config_type_error(key, "array of {action,target} objects"))?;
    let mut output = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let Some(entry) = item.as_object() else {
            return Err(config_value_error(
                key,
                &format!("entry {index} must be an object"),
            ));
        };
        if let Some(unknown) = entry
            .keys()
            .find(|field| !matches!(field.as_str(), "action" | "target"))
        {
            return Err(config_value_error(
                key,
                &format!("entry {index} has unknown field '{unknown}'"),
            ));
        }
        let action = entry
            .get("action")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_value_error(key, &format!("entry {index} requires action")))?;
        let target = entry
            .get("target")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_value_error(key, &format!("entry {index} requires target")))?;
        output.push(RuntimeCloudWanAllowRule {
            action: SmolStr::new(action),
            target: SmolStr::new(target),
        });
    }
    Ok(output)
}

fn expect_link_preference_rules(
    key: &str,
    value: &serde_json::Value,
) -> Result<Vec<RuntimeCloudLinkPreferenceRule>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| config_type_error(key, "array of {source,target,transport} objects"))?;
    let mut output = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let Some(entry) = item.as_object() else {
            return Err(config_value_error(
                key,
                &format!("entry {index} must be an object"),
            ));
        };
        if let Some(unknown) = entry
            .keys()
            .find(|field| !matches!(field.as_str(), "source" | "target" | "transport"))
        {
            return Err(config_value_error(
                key,
                &format!("entry {index} has unknown field '{unknown}'"),
            ));
        }
        let source = entry
            .get("source")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_value_error(key, &format!("entry {index} requires source")))?;
        let target = entry
            .get("target")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_value_error(key, &format!("entry {index} requires target")))?;
        let transport_raw = entry
            .get("transport")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_value_error(key, &format!("entry {index} requires transport")))?;
        let transport = RuntimeCloudPreferredTransport::parse(transport_raw)
            .map_err(|error| config_value_error(key, &error.to_string()))?;
        output.push(RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new(source),
            target: SmolStr::new(target),
            transport,
        });
    }
    Ok(output)
}
