use super::*;

impl IoToml {
    pub(crate) fn into_config(self) -> Result<IoConfig, RuntimeError> {
        let legacy_form_present = self.io.driver.is_some() || self.io.params.is_some();
        let legacy_driver = self
            .io
            .driver
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned);
        let legacy_params = self
            .io
            .params
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        let explicit_drivers = self.io.drivers.unwrap_or_default();

        if legacy_form_present && !explicit_drivers.is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "use either io.driver/io.params or io.drivers, not both".into(),
            ));
        }

        let drivers = if let Some(driver) = legacy_driver {
            if !legacy_params.is_table() {
                return Err(RuntimeError::InvalidConfig(
                    "io.params must be a table".into(),
                ));
            }
            vec![IoDriverConfig {
                name: SmolStr::new(driver),
                params: legacy_params,
                enabled: true,
            }]
        } else {
            if explicit_drivers.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "io.driver or io.drivers must be set".into(),
                ));
            }
            explicit_drivers
                .into_iter()
                .enumerate()
                .map(|(idx, driver)| {
                    if driver.name.trim().is_empty() {
                        return Err(RuntimeError::InvalidConfig(
                            format!("io.drivers[{idx}].name must not be empty").into(),
                        ));
                    }
                    let params = driver
                        .params
                        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
                    if !params.is_table() {
                        return Err(RuntimeError::InvalidConfig(
                            format!("io.drivers[{idx}].params must be a table").into(),
                        ));
                    }
                    Ok(IoDriverConfig {
                        name: SmolStr::new(driver.name),
                        params,
                        enabled: driver.enabled.unwrap_or(true),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut safe_state = IoSafeState::default();
        if let Some(entries) = self.io.safe_state {
            for entry in entries {
                let address = IoAddress::parse(&entry.address)?;
                let value = parse_io_value(&entry.value, address.size)?;
                safe_state.outputs.push((address, value));
            }
        }
        Ok(IoConfig {
            drivers,
            safe_state,
        })
    }
}

fn parse_io_value(text: &str, size: IoSize) -> Result<Value, RuntimeError> {
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    match size {
        IoSize::Bit => match upper.as_str() {
            "TRUE" | "1" => Ok(Value::Bool(true)),
            "FALSE" | "0" => Ok(Value::Bool(false)),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid BOOL safe_state value '{trimmed}'").into(),
            )),
        },
        IoSize::Byte => Ok(Value::Byte(
            parse_bounded_u64(trimmed, u8::MAX.into(), "BYTE")? as u8,
        )),
        IoSize::Word => Ok(Value::Word(
            parse_bounded_u64(trimmed, u16::MAX.into(), "WORD")? as u16,
        )),
        IoSize::DWord => Ok(Value::DWord(
            parse_bounded_u64(trimmed, u32::MAX.into(), "DWORD")? as u32,
        )),
        IoSize::LWord => Ok(Value::LWord(parse_u64(trimmed)?)),
        IoSize::Bytes(len) => {
            let text = trimmed.trim_matches('\'');
            if text.len() > len as usize {
                return Err(RuntimeError::InvalidConfig(
                    "STRING safe-state value is too long".into(),
                ));
            }
            Ok(Value::String(text.into()))
        }
    }
}

fn parse_bounded_u64(text: &str, maximum: u64, type_name: &str) -> Result<u64, RuntimeError> {
    let value = parse_u64(text)?;
    if value > maximum {
        return Err(RuntimeError::InvalidConfig(
            format!("{type_name} safe_state value '{text}' out of range 0..={maximum}").into(),
        ));
    }
    Ok(value)
}

fn parse_u64(text: &str) -> Result<u64, RuntimeError> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).map_err(|err| {
            RuntimeError::InvalidConfig(format!("invalid hex value '{trimmed}': {err}").into())
        });
    }
    trimmed.parse::<u64>().map_err(|err| {
        RuntimeError::InvalidConfig(format!("invalid numeric value '{trimmed}': {err}").into())
    })
}
