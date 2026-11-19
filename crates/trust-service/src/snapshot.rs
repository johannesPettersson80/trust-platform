//! One completed scan's values of a set of variables (specification 11 §6.8.1, "Reading
//! variables"; specification 48 RU-62): the scan's number, its logical time, the resource's state
//! and the quality, and each value in the JSON form of its declared type, which [`Snapshot::get`]
//! reads as a Rust value.

use std::time::Duration;

use serde_json::Value;

use crate::ServiceError;

/// Whether a snapshot's values are current.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// The resource runs: the values are the last completed scan's.
    Good,
    /// The resource does not run: the values are those the last completed scan left.
    Stale,
}

/// One variable of a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// Its path, as the read named it.
    pub path: String,
    /// Its declared type, as the program declares it: `REAL`, `STRING[20]`, `Color`.
    pub type_name: String,
    /// Its value, in the JSON form of its type.
    pub value: Value,
}

/// One completed scan's values of the variables a read named, in the read's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The number of the completed scan.
    pub scan: u64,
    /// Its logical time in nanoseconds (`TIME()`).
    pub time_ns: i64,
    /// The resource's state when the runtime answered, as `status` names it: `running`.
    pub state: String,
    /// Whether the values are current.
    pub quality: Quality,
    /// The variables, in the read's order.
    pub values: Vec<Variable>,
}

impl Snapshot {
    /// The snapshot of a `variables.read` reply.
    pub(crate) fn of_reply(reply: &Value) -> Option<Self> {
        let quality = match reply.get("quality")?.as_str()? {
            "good" => Quality::Good,
            "stale" => Quality::Stale,
            _ => return None,
        };
        let values = reply
            .get("values")?
            .as_array()?
            .iter()
            .map(|entry| {
                Some(Variable {
                    path: entry.get("path")?.as_str()?.to_string(),
                    type_name: entry.get("type")?.as_str()?.to_string(),
                    value: entry.get("value")?.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            scan: reply.get("scan")?.as_u64()?,
            time_ns: reply.get("time_ns")?.as_i64()?,
            state: reply.get("state")?.as_str()?.to_string(),
            quality,
            values,
        })
    }

    /// The value of `variable`, which the read named, as its Rust type.
    ///
    /// # Errors
    ///
    /// As [`Self::get`].
    pub fn value<T: FromVariable>(&self, variable: crate::Variable<T>) -> Result<T, ServiceError> {
        self.get(variable.path())
    }

    /// The value of the variable the read named `path`, as a `T`.
    ///
    /// # Errors
    ///
    /// [`ServiceError::NotRead`] when the read did not name `path`, and
    /// [`ServiceError::Type`] when its value is no `T`: of another type, or outside `T`'s range.
    pub fn get<T: FromVariable>(&self, path: &str) -> Result<T, ServiceError> {
        let variable = self
            .values
            .iter()
            .find(|variable| variable.path == path)
            .ok_or_else(|| ServiceError::NotRead(path.to_string()))?;
        T::from_variable(&variable.type_name, &variable.value).ok_or_else(|| ServiceError::Type {
            path: path.to_string(),
            type_name: variable.type_name.clone(),
            wanted: T::NAME,
        })
    }
}

/// A Rust value that a variable's value reads as.
pub trait FromVariable: Sized {
    /// The Rust type's name, for the refusal of a value that is none.
    const NAME: &'static str;

    /// `value`, of the declared type `type_name`, as `Self`; `None` when it is none.
    fn from_variable(type_name: &str, value: &Value) -> Option<Self>;
}

impl FromVariable for bool {
    const NAME: &'static str = "bool";
    fn from_variable(_type_name: &str, value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

macro_rules! integer {
    ($($ty:ty),*) => {$(
        impl FromVariable for $ty {
            const NAME: &'static str = stringify!($ty);
            fn from_variable(_type_name: &str, value: &Value) -> Option<Self> {
                value
                    .as_i64()
                    .and_then(|number| <$ty>::try_from(number).ok())
                    .or_else(|| value.as_u64().and_then(|number| <$ty>::try_from(number).ok()))
            }
        }
    )*};
}

integer!(i8, i16, i32, i64, u8, u16, u32, u64);

/// A `REAL`'s or an `LREAL`'s value: a JSON number, or `NaN`, `Infinity` or `-Infinity`.
fn real(value: &Value) -> Option<f64> {
    match value.as_str() {
        Some("NaN") => Some(f64::NAN),
        Some("Infinity") => Some(f64::INFINITY),
        Some("-Infinity") => Some(f64::NEG_INFINITY),
        Some(_) => None,
        None => value.as_f64(),
    }
}

impl FromVariable for f32 {
    const NAME: &'static str = "f32";
    /// A `REAL` alone: an `LREAL` holds values no `f32` holds.
    // The shortest text of a REAL reads back as the same single-precision value.
    #[allow(clippy::cast_possible_truncation)]
    fn from_variable(type_name: &str, value: &Value) -> Option<Self> {
        type_name
            .eq_ignore_ascii_case("REAL")
            .then(|| real(value))
            .flatten()
            .map(|number| number as f32)
    }
}

impl FromVariable for f64 {
    const NAME: &'static str = "f64";
    fn from_variable(type_name: &str, value: &Value) -> Option<Self> {
        (type_name.eq_ignore_ascii_case("REAL") || type_name.eq_ignore_ascii_case("LREAL"))
            .then(|| real(value))
            .flatten()
    }
}

impl FromVariable for String {
    const NAME: &'static str = "String";
    /// A string's, a character's or an enumerator's text.
    fn from_variable(_type_name: &str, value: &Value) -> Option<Self> {
        value.as_str().map(str::to_string)
    }
}

impl FromVariable for Duration {
    const NAME: &'static str = "Duration";
    /// A `TIME`'s or an `LTIME`'s nanoseconds that are not negative.
    fn from_variable(type_name: &str, value: &Value) -> Option<Self> {
        (type_name.eq_ignore_ascii_case("TIME") || type_name.eq_ignore_ascii_case("LTIME"))
            .then(|| value.as_u64())
            .flatten()
            .map(Duration::from_nanos)
    }
}

impl FromVariable for Value {
    const NAME: &'static str = "a JSON value";
    /// The value in the JSON form of its type, whatever the type.
    fn from_variable(_type_name: &str, value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
