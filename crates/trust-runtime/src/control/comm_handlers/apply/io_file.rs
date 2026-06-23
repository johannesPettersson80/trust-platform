use std::path::Path;

use toml_edit::{value, Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use super::{field_error, CommFieldError};
use crate::config::IoDriverConfig;

pub(super) fn render_io_toml(
    path: &Path,
    drivers: &[IoDriverConfig],
    safe_state: &[(String, String)],
) -> Result<String, CommFieldError> {
    let mut doc = load_io_doc(path)?;
    let io = ensure_table(doc.as_table_mut(), "io")?;
    io.remove("driver");
    io.remove("params");
    io.insert(
        "safe_state",
        Item::Value(Value::Array(safe_state_array(safe_state))),
    );
    io.insert("drivers", Item::ArrayOfTables(driver_tables(drivers)));
    Ok(doc.to_string())
}

fn load_io_doc(path: &Path) -> Result<DocumentMut, CommFieldError> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .map_err(|error| field_error("io.toml", format!("invalid TOML: {error}"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(error) => Err(field_error(
            "io.toml",
            format!("failed to read {}: {error}", path.display()),
        )),
    }
}

fn ensure_table<'a>(parent: &'a mut Table, key: &str) -> Result<&'a mut Table, CommFieldError> {
    if !parent.contains_key(key) {
        parent.insert(key, Item::Table(Table::new()));
    }
    parent
        .get_mut(key)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| field_error(key, "Expected a TOML table."))
}

fn safe_state_array(safe_state: &[(String, String)]) -> Array {
    let mut array = Array::new();
    for (address, output_value) in safe_state {
        let mut entry = InlineTable::new();
        entry.insert("address", Value::from(address.as_str()));
        entry.insert("value", Value::from(output_value.as_str()));
        array.push(Value::InlineTable(entry));
    }
    array
}

fn driver_tables(drivers: &[IoDriverConfig]) -> ArrayOfTables {
    let mut tables = ArrayOfTables::new();
    for driver in drivers {
        let mut table = Table::new();
        table.insert("name", value(driver.name.as_str()));
        table.insert("params", params_item(&driver.params));
        tables.push(table);
    }
    tables
}

fn params_item(params: &toml::Value) -> Item {
    match params {
        toml::Value::Table(table) => Item::Value(Value::InlineTable(inline_table_from_toml(table))),
        value => Item::Value(edit_value_from_toml(value)),
    }
}

fn inline_table_from_toml(table: &toml::map::Map<String, toml::Value>) -> InlineTable {
    let mut inline = InlineTable::new();
    for (key, value) in table {
        inline.insert(key, edit_value_from_toml(value));
    }
    inline
}

fn edit_value_from_toml(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(value) => Value::from(value.as_str()),
        toml::Value::Integer(value) => Value::from(*value),
        toml::Value::Float(value) => Value::from(*value),
        toml::Value::Boolean(value) => Value::from(*value),
        toml::Value::Datetime(value) => Value::from(value.to_string()),
        toml::Value::Array(values) => {
            let mut array = Array::new();
            for value in values {
                array.push(edit_value_from_toml(value));
            }
            Value::Array(array)
        }
        toml::Value::Table(table) => Value::InlineTable(inline_table_from_toml(table)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use smol_str::SmolStr;

    use super::*;

    #[test]
    fn render_io_toml_adds_nested_drivers_to_safe_state_only_file() {
        let root = std::env::temp_dir().join(format!(
            "trust-comm-io-file-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("io.toml");
        fs::write(
            &path,
            r#"
# setup note
[io]
safe_state = [{ address = "%QX0.0", value = "FALSE" }]
"#,
        )
        .expect("write input io.toml");

        let params = toml::toml! {
            address = "127.0.0.1:1502"
            unit_id = 1
            input_start = 0
            output_start = 0
            timeout_ms = 500
            on_error = "warn"
        };
        let rendered = render_io_toml(
            &path,
            &[IoDriverConfig {
                name: SmolStr::new("modbus-tcp"),
                params: toml::Value::Table(params),
            }],
            &[("%QX0.0".to_string(), "FALSE".to_string())],
        )
        .expect("render io.toml");

        assert!(rendered.contains("setup note"), "{rendered}");
        assert!(rendered.contains("[[io.drivers]]"), "{rendered}");
        crate::config::validate_io_toml_text(&rendered).expect("rendered io.toml should validate");
        fs::remove_dir_all(root).ok();
    }
}
