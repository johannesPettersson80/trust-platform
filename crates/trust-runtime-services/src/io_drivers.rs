//! The `analog`, `pwm`, `counter` and `can` I/O drivers that every runtime shares (specification 11
//! §6.5; specification 34 TARGET-01): their parameters with the keys, defaults and checks of
//! `io.toml`, and the logic that does not depend on the hardware. A chip reads the parameters from
//! its record's section 8 (specification 34 §5.1.1), a computer from its `io.toml`, so both refuse
//! and accept the same files; each runtime's hardware layer then claims its lines and calls the
//! logic here at each cycle.

pub mod analog;
pub mod can;
pub mod counter;
pub mod pwm;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde_json::{Map, Value};
use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

/// Why a driver's parameters are refused: the key, as a path such as `outputs[1].frequency_hz`,
/// and the reason. Its text is `<key>: <reason>`, which the build, `status` and a chip's refusal
/// put after the driver's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamsError {
    /// The key the reason is about, as a path from the parameter table's root.
    pub key: String,
    /// What is wrong with it.
    pub reason: String,
}

impl fmt::Display for ParamsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.key, self.reason)
    }
}

/// What a driver error does to the resource (specification 11 §6.5, as for Modbus): `fault` stops
/// it, `warn` keeps it running with the driver's health degraded, `ignore` keeps it running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OnError {
    /// The resource faults.
    #[default]
    Fault,
    /// The resource runs on; the driver's health is degraded.
    Warn,
    /// The resource runs on; the error is suppressed.
    Ignore,
}

/// The process-image bytes that one entry of a driver takes (specification 11 §6.5): its area and
/// the byte offsets from its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryBytes {
    /// The image the entry reads into or writes from.
    pub area: IoArea,
    /// The bytes it takes.
    pub bytes: core::ops::Range<u32>,
}

/// The bytes that the entries of the driver `name` with the parameters `params` take: a word for
/// each analog input and output and each PWM output, a double word for each counter, and its
/// `length` bytes for each CAN frame; `None` for a driver other than these four.
///
/// # Errors
///
/// The [`ParamsError`] of parameters the driver refuses.
#[must_use]
pub fn entry_bytes(name: &str, params: &str) -> Option<Result<Vec<EntryBytes>, ParamsError>> {
    fn at(address: &IoAddress, len: u32) -> EntryBytes {
        EntryBytes {
            area: address.area,
            bytes: address.byte..address.byte.saturating_add(len),
        }
    }
    Some(match name {
        "analog" => analog::AnalogParams::from_json(params).map(|driver| {
            let inputs = driver.inputs.iter().map(|entry| at(&entry.address, 2));
            let outputs = driver.outputs.iter().map(|entry| at(&entry.address, 2));
            inputs.chain(outputs).collect()
        }),
        "pwm" => pwm::PwmParams::from_json(params).map(|driver| {
            driver
                .outputs
                .iter()
                .map(|entry| at(&entry.address, 2))
                .collect()
        }),
        "counter" => counter::CounterParams::from_json(params).map(|driver| {
            driver
                .inputs
                .iter()
                .map(|entry| at(&entry.address, 4))
                .collect()
        }),
        "can" => can::CanParams::from_json(params).map(|driver| {
            let receive = driver
                .receive
                .iter()
                .map(|entry| at(&entry.address, u32::from(entry.length)));
            let transmit = driver
                .transmit
                .iter()
                .map(|entry| at(&entry.address, u32::from(entry.length)));
            receive.chain(transmit).collect()
        }),
        _ => return None,
    })
}

/// The key a refusal of the parameter table itself names: the table's own name in `io.toml`.
const ROOT: &str = "params";

/// The rule a refusal of a line used twice states.
const LINE_ONCE: &str = "a line is used once";

/// The rule a refusal of a process-image address used twice in one list states.
const ADDRESS_ONCE: &str = "the entries of a list use bytes of their own";

/// What a `line` must be.
const LINE: &str = "a non-empty string naming the line";

impl ParamsError {
    /// The refusal of `key` for `reason`.
    fn new(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            reason: reason.into(),
        }
    }
}

/// The parameter table of the JSON text `text`, the driver's `params` (specification 34 §5.1.1).
fn parse_root(text: &str) -> Result<Map<String, Value>, ParamsError> {
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => Err(ParamsError::new(
            ROOT,
            format!("must be a table, not {other}"),
        )),
        Err(error) => Err(ParamsError::new(ROOT, format!("is not JSON: {error}"))),
    }
}

/// The address an entry takes, and how a refusal writes it.
struct AddressKind {
    /// The process image's area.
    area: IoArea,
    /// The size.
    size: IoSize,
    /// The address's prefix, such as `%IW`.
    prefix: &'static str,
    /// What the size is called, such as `word`.
    noun: &'static str,
}

/// A table of the parameters, with its path from the root that a refusal names.
struct Table<'a> {
    /// Empty at the root, and the list's key with the entry's index in a list, such as `inputs[0]`.
    path: String,
    /// The table's keys and values.
    map: &'a Map<String, Value>,
}

impl<'a> Table<'a> {
    /// The root table, `map`.
    fn root(map: &'a Map<String, Value>) -> Self {
        Self {
            path: String::new(),
            map,
        }
    }

    /// The path of `key` in this table: `key` at the root, such as `inputs[0].key` in a list.
    fn key(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.into()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    /// The refusal of `key` of this table for `reason`.
    fn refuse(&self, key: &str, reason: impl Into<String>) -> ParamsError {
        ParamsError::new(self.key(key), reason)
    }

    /// The value of `key`; `None` when the key is absent.
    fn get(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key)
    }

    /// Refuses the first key that is not one of `keys`, all the keys that `what` takes.
    fn only(&self, keys: &[&str], what: &str) -> Result<(), ParamsError> {
        if let Some(key) = self.map.keys().find(|key| !keys.contains(&key.as_str())) {
            return Err(self.refuse(
                key,
                format!("unknown key; {what} takes {}", listing(keys, " and ")),
            ));
        }
        Ok(())
    }

    /// The tables of the list `key`, in order; an absent list is empty.
    fn list(&self, key: &str) -> Result<Vec<Table<'a>>, ParamsError> {
        let Some(value) = self.get(key) else {
            return Ok(Vec::new());
        };
        let Value::Array(items) = value else {
            return Err(self.refuse(key, format!("must be an array of tables, not {value}")));
        };
        let list = self.key(key);
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let path = format!("{list}[{index}]");
                match item {
                    Value::Object(map) => Ok(Table { path, map }),
                    other => Err(ParamsError::new(
                        path,
                        format!("must be a table, not {other}"),
                    )),
                }
            })
            .collect()
    }

    /// `value`, which `key` gives as `what`; refused when the key is absent.
    fn required<T>(&self, key: &str, what: &str, value: Option<T>) -> Result<T, ParamsError> {
        value.ok_or_else(|| self.refuse(key, format!("is required, {what}")))
    }

    /// The value of `key` when the key is given, which must be `what`, a non-empty string.
    fn text(&self, key: &str, what: &str) -> Result<Option<SmolStr>, ParamsError> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::String(text)) if !text.is_empty() => Ok(Some(SmolStr::new(text))),
            Some(other) => Err(self.refuse(key, format!("must be {what}, not {other}"))),
        }
    }

    /// The line `key` names when the key is given, `what`; refused when an earlier key of the
    /// driver names it (`lines`).
    fn line(
        &self,
        key: &str,
        what: &str,
        lines: &mut Once<SmolStr>,
    ) -> Result<Option<SmolStr>, ParamsError> {
        let Some(line) = self.text(key, what)? else {
            return Ok(None);
        };
        lines.claim(line.clone(), self.key(key), &quoted(&line))?;
        Ok(Some(line))
    }

    /// The line `key` names, read as `line` reads it, which is required.
    fn required_line(
        &self,
        key: &str,
        what: &str,
        lines: &mut Once<SmolStr>,
    ) -> Result<SmolStr, ParamsError> {
        let line = self.line(key, what, lines)?;
        self.required(key, what, line)
    }

    /// The integer `key` gives, from `min` to `max`, when the key is given.
    fn integer<T>(&self, key: &str, min: T, max: T) -> Result<Option<T>, ParamsError>
    where
        T: Copy + PartialOrd + fmt::Display + TryFrom<i64>,
    {
        let Some(value) = self.get(key) else {
            return Ok(None);
        };
        match value.as_i64().and_then(|number| T::try_from(number).ok()) {
            Some(number) if (min..=max).contains(&number) => Ok(Some(number)),
            _ => Err(self.refuse(
                key,
                format!("must be {}, not {value}", integer_text(min, max)),
            )),
        }
    }

    /// The integer `key` gives, from `min` to `max`, which is required.
    fn required_integer<T>(&self, key: &str, min: T, max: T) -> Result<T, ParamsError>
    where
        T: Copy + PartialOrd + fmt::Display + TryFrom<i64>,
    {
        let number = self.integer(key, min, max)?;
        self.required(key, &integer_text(min, max), number)
    }

    /// `true` or `false`, when `key` is given.
    fn boolean(&self, key: &str) -> Result<Option<bool>, ParamsError> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::Bool(flag)) => Ok(Some(*flag)),
            Some(other) => Err(self.refuse(key, format!("must be true or false, not {other}"))),
        }
    }

    /// The value of the word `key` gives, one of the words of `choices`, when the key is given.
    fn choice<T: Copy>(&self, key: &str, choices: &[(&str, T)]) -> Result<Option<T>, ParamsError> {
        let Some(value) = self.get(key) else {
            return Ok(None);
        };
        if let Some((_, choice)) = value
            .as_str()
            .and_then(|text| choices.iter().find(|(word, _)| *word == text))
        {
            return Ok(Some(*choice));
        }
        let words: Vec<String> = choices.iter().map(|(word, _)| quoted(word)).collect();
        Err(self.refuse(
            key,
            format!("must be {}, not {value}", listing(&words, " or ")),
        ))
    }

    /// The entry's `address`: concrete, flat and of `kind`. The list's [`Ranges`] then take its
    /// bytes.
    fn address(&self, kind: &AddressKind) -> Result<IoAddress, ParamsError> {
        let what = format!(
            "a concrete, flat {} {} such as \"{}0\"",
            kind.prefix, kind.noun, kind.prefix
        );
        let Some(value) = self.get("address") else {
            return Err(self.refuse("address", format!("is required, {what}")));
        };
        let address = value
            .as_str()
            .and_then(|text| IoAddress::parse(text).ok())
            .filter(|address| {
                !address.wildcard
                    && address.path.len() == 1
                    && address.area == kind.area
                    && address.size == kind.size
                    && address.flat_byte_end().is_ok()
            })
            .ok_or_else(|| self.refuse("address", format!("must be {what}, not {value}")))?;
        Ok(address)
    }
}

/// The process-image bytes that the entries of one list use, each with the key and the address that
/// use them: an entry's bytes may not overlap another's, so that two entries never write the same
/// byte (specification 11 §6.5). `%IW0` takes bytes 0 and 1, so `%IW1` overlaps it and `%IW2` does
/// not; an entry of no bytes, a CAN frame of length 0, overlaps nothing.
struct Ranges {
    used: Vec<(core::ops::Range<u32>, String, String)>,
}

impl Ranges {
    /// No bytes used yet.
    fn new() -> Self {
        Self { used: Vec::new() }
    }

    /// Records that the entry `table` uses the `bytes` bytes from `address`, which a refusal writes
    /// with `prefix`; refused when an earlier entry of the list uses one of them.
    fn claim(
        &mut self,
        table: &Table<'_>,
        address: &IoAddress,
        bytes: u32,
        prefix: &str,
    ) -> Result<(), ParamsError> {
        let range = address.byte..address.byte.saturating_add(bytes);
        let text = format!("\"{prefix}{}\"", address.byte);
        if let Some((_, key, earlier)) = self
            .used
            .iter()
            .find(|(used, _, _)| used.start < range.end && range.start < used.end)
        {
            return Err(table.refuse(
                "address",
                format!("{text} overlaps the bytes of {key} ({earlier}); {ADDRESS_ONCE}"),
            ));
        }
        self.used.push((range, table.key("address"), text));
        Ok(())
    }
}

/// What a driver's entries use, each with the key that uses it, where each may be used once: a
/// line in the driver, or an address or a frame identifier in one list.
struct Once<T> {
    /// The rule a refusal states.
    rule: &'static str,
    /// What is used so far, with the key that uses it.
    used: Vec<(T, String)>,
}

impl<T: PartialEq> Once<T> {
    /// Nothing used yet, under `rule`.
    fn new(rule: &'static str) -> Self {
        Self {
            rule,
            used: Vec::new(),
        }
    }

    /// Records that `key` uses `item`, which a refusal writes as `text`; refused when an earlier
    /// key uses it.
    fn claim(&mut self, item: T, key: String, text: &str) -> Result<(), ParamsError> {
        if let Some((_, earlier)) = self.used.iter().find(|(used, _)| *used == item) {
            return Err(ParamsError::new(
                key,
                format!("{text} is used by {earlier} already; {}", self.rule),
            ));
        }
        self.used.push((item, key));
        Ok(())
    }
}

/// `text` as a JSON string, the way a refusal quotes a value.
fn quoted(text: &str) -> String {
    Value::from(text).to_string()
}

/// The words of an integer from `min` to `max`.
fn integer_text<T: fmt::Display + Copy>(min: T, max: T) -> String {
    format!("an integer from {min} to {max}")
}

/// `items` in words, with `last` before the last item: `a`, `a and b`, `a, b and c`.
fn listing<T: AsRef<str>>(items: &[T], last: &str) -> String {
    let mut text = String::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            text.push_str(if index + 1 == items.len() { last } else { ", " });
        }
        text.push_str(item.as_ref());
    }
    text
}
