fn is_config_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| CONFIG_FILES.iter().any(|candidate| candidate == &name))
        .unwrap_or(false)
}

fn is_hmi_toml_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
        return false;
    }
    path.components()
        .any(|component| component.as_os_str() == "hmi")
}
