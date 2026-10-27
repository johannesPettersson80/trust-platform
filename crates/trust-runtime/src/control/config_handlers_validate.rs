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
