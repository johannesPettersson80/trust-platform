//! Credential-safe I/O text editing. Stored text stays private and owns revision identity.

use super::{RuntimeError, SECRET_MARKER};
use toml_edit::{DocumentMut, Item};

pub(super) fn is_io_config(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("io.toml"))
}

pub(super) fn invalid_io_config() -> RuntimeError {
    RuntimeError::InvalidConfig("invalid I/O configuration; inspect the local file".into())
}

fn parse(text: &str) -> Result<DocumentMut, RuntimeError> {
    // TOML parse errors contain the source line, which may contain credentials.
    text.parse().map_err(|_| invalid_io_config())
}

pub(super) fn redact(text: &str) -> Result<String, RuntimeError> {
    let mut document = parse(text)?;
    transform_io(document.as_item_mut(), None, false)?;
    Ok(document.to_string())
}

pub(super) fn restore(text: &str, stored: &str) -> Result<String, RuntimeError> {
    let mut document = parse(text)?;
    let stored = parse(stored).ok();
    transform_io(
        document.as_item_mut(),
        stored.as_ref().map(DocumentMut::as_item),
        true,
    )?;
    let text = document.to_string();
    crate::config::validate_io_toml_text(&text).map_err(|_| invalid_io_config())?;
    Ok(text)
}

fn driver_name(item: &Item) -> Option<&str> {
    item.get("name")
        .or_else(|| item.get("driver"))
        .and_then(Item::as_str)
}

fn transform_io(
    document: &mut Item,
    stored: Option<&Item>,
    restore: bool,
) -> Result<(), RuntimeError> {
    if document.get("io").is_none() {
        return Ok(());
    }
    let Some(io) = document.get_mut("io") else {
        return Ok(());
    };
    let stored = stored.and_then(|document| document.get("io"));
    let same = stored.filter(|stored| {
        io.get("driver").and_then(Item::as_str) == stored.get("driver").and_then(Item::as_str)
    });
    if let Some(params) = io.get_mut("params") {
        transform(params, same.and_then(|io| io.get("params")), restore)?;
    }
    if let Some(drivers) = io.get_mut("drivers") {
        let stored = stored.and_then(|io| io.get("drivers"));
        let mut index = 0;
        while let Some(driver) = drivers.get_mut(index) {
            let same = stored
                .and_then(|drivers| drivers.get(index))
                .filter(|stored| driver_name(driver) == driver_name(stored));
            if let Some(params) = driver.get_mut("params") {
                transform(
                    params,
                    same.and_then(|driver| driver.get("params")),
                    restore,
                )?;
            }
            index += 1;
        }
    }
    Ok(())
}

fn transform(item: &mut Item, stored: Option<&Item>, restore: bool) -> Result<(), RuntimeError> {
    if let Some(table) = item.as_table_like_mut() {
        for (key, child) in table.iter_mut() {
            let kept = stored.and_then(|stored| stored.get(key.get()));
            if crate::security::is_secret_param_key(key.get()) {
                let replacement = if !restore {
                    Some(toml_edit::Value::from(SECRET_MARKER))
                } else if child.as_str() == Some(SECRET_MARKER) {
                    Some(
                        kept.cloned()
                            .and_then(|value| value.into_value().ok())
                            .ok_or_else(|| {
                                RuntimeError::InvalidConfig(
                                    "redacted I/O secret has no stored value".into(),
                                )
                            })?,
                    )
                } else {
                    None
                };
                if let Some(mut value) = replacement {
                    if let Some(previous) = child.as_value() {
                        *value.decor_mut() = previous.decor().clone();
                    }
                    *child = Item::Value(value);
                }
            } else {
                transform(child, kept, restore)?;
            }
        }
    } else {
        let mut index = 0;
        while let Some(child) = item.get_mut(index) {
            transform(child, stored.and_then(|stored| stored.get(index)), restore)?;
            index += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
