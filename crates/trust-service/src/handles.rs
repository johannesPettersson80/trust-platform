//! Typed handles of the program's variables, as a service's bindings name them (specification 48
//! RU-62): a [`Variable`] is read as its Rust type, and a [`Writable`], a variable the service's
//! `writes` names, can also be written, so that the compiler checks a service's paths and types
//! against the program, and a service whose code writes another variable does not compile.

use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;

use serde_json::{json, Value};

/// A variable of the program, by its path, read as a `T`.
pub struct Variable<T> {
    path: &'static str,
    value: PhantomData<fn() -> T>,
}

impl<T> Variable<T> {
    /// The variable at `path`, as the program names it: `P1.level`.
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self {
            path,
            value: PhantomData,
        }
    }

    /// Its path.
    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.path
    }
}

impl<T> Clone for Variable<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Variable<T> {}

impl<T> fmt::Debug for Variable<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Variable").field(&self.path).finish()
    }
}

/// A variable the service's `writes` names, which it reads and writes as a `T`.
pub struct Writable<T>(Variable<T>);

impl<T> Writable<T> {
    /// The writable variable at `path`.
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self(Variable::new(path))
    }

    /// The variable, to read it.
    #[must_use]
    pub const fn variable(&self) -> Variable<T> {
        Variable::new(self.0.path)
    }

    /// Its path.
    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.0.path
    }
}

impl<T> Clone for Writable<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Writable<T> {}

impl<T> fmt::Debug for Writable<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Writable").field(&self.0.path).finish()
    }
}

/// A Rust value written as the JSON form of its variable's type (specification 11 §6.8.1, "Writing
/// variables"), the inverse of [`crate::FromVariable`].
pub trait ToVariable {
    /// The value's JSON form.
    fn to_variable(&self) -> Value;
}

macro_rules! plain {
    ($($ty:ty),*) => {$(
        impl ToVariable for $ty {
            fn to_variable(&self) -> Value {
                json!(self)
            }
        }
    )*};
}

plain!(bool, i8, i16, i32, i64, u8, u16, u32, u64, String, &str);

/// A real number as a JSON number, or `NaN`, `Infinity` or `-Infinity`, which no JSON number holds.
fn real(number: f64) -> Value {
    if number.is_nan() {
        json!("NaN")
    } else if number.is_infinite() {
        json!(if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        })
    } else {
        json!(number)
    }
}

impl ToVariable for f32 {
    /// The single-precision value's shortest text, which the runtime reads back as the same value.
    fn to_variable(&self) -> Value {
        match self.to_string().parse::<f64>() {
            Ok(number) if self.is_finite() => real(number),
            _ => real(f64::from(*self)),
        }
    }
}

impl ToVariable for f64 {
    fn to_variable(&self) -> Value {
        real(*self)
    }
}

impl ToVariable for Duration {
    /// A `TIME`'s or an `LTIME`'s nanoseconds; a duration longer than `i64::MAX` nanoseconds,
    /// about 292 years, is written as that, which the runtime refuses for a `TIME`.
    fn to_variable(&self) -> Value {
        json!(i64::try_from(self.as_nanos()).unwrap_or(i64::MAX))
    }
}

#[cfg(test)]
#[path = "handles_tests.rs"]
mod tests;
