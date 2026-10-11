//! Shared IEC standard functions and storage-backed function blocks.

use alloc::borrow::Cow;
pub(crate) mod assertions;
pub(crate) mod bit;
pub(crate) mod comparison;
/// Typed conversion recognition and execution.
pub mod conversions;
/// Portable standard function blocks with caller-owned time and storage.
pub mod fbs;
pub(crate) mod helpers;
/// Compatibility exports for the source-backed hosted standard library.
#[cfg(feature = "hir")]
pub mod hosted;
pub(crate) mod numeric;
mod parameters;
mod registration;
pub(crate) mod selection;
pub(crate) mod string;
/// Date/time standard-function bindings and supplied-clock adapters.
pub mod time;
pub(crate) mod validate;

use crate::collections::OrderedMap as IndexMap;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::value::Value;

/// Standard function signature.
pub type StdFunc = fn(&[Value]) -> Result<Value, RuntimeError>;

/// Standard function parameter specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdParams {
    /// Fixed parameter list, borrowed for built-ins and owned for custom registration.
    Fixed(Cow<'static, [SmolStr]>),
    /// Variadic parameters with a numeric suffix (e.g., IN1, IN2, ...).
    Variadic {
        /// Fixed parameter names that appear before the variadic set.
        fixed: Cow<'static, [SmolStr]>,
        /// Prefix for variadic parameters (e.g., "IN").
        prefix: SmolStr,
        /// Starting index for variadic parameters.
        start: usize,
        /// Minimum count of variadic parameters.
        min: usize,
    },
}

/// Standard function metadata.
#[derive(Debug, Clone)]
pub struct StdFunction {
    /// Parameter names (uppercase).
    pub params: StdParams,
    /// Function implementation.
    pub func: StdFunc,
}

/// Borrowed standard-function metadata returned by [`StandardLibrary::get`].
///
/// Built-ins borrow shared immutable signatures. Custom entries borrow the
/// registry's owned metadata. Clone `params` when a caller must mutate the runtime
/// while binding arguments; cloning built-in signatures does not allocate.
#[derive(Debug, Clone, Copy)]
pub struct StdFunctionRef<'a> {
    /// Parameter names and shape, borrowed from their owning registry or static table.
    pub params: &'a StdParams,
    /// Function implementation, identical for borrowed and owned registration.
    pub func: StdFunc,
}

/// Standard library registry for functions/FBs.
///
/// [`Self::new`] shares immutable built-in descriptors without allocating them.
/// [`Self::default`] remains an empty registry. Registrations are private to each
/// library and replace built-ins using the same ASCII-insensitive names.
#[derive(Debug, Default, Clone)]
pub struct StandardLibrary {
    functions: IndexMap<SmolStr, StdFunction>,
    defaults: bool,
}

impl StandardLibrary {
    /// Build a standard library with default functions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: IndexMap::default(),
            defaults: true,
        }
    }

    /// Shared descriptors require no per-module heap allocation or registry construction.
    pub(crate) fn preparation_demand() -> (usize, usize) {
        (0, 1)
    }

    fn register_descriptors(
        &mut self,
        descriptors: &'static [(&'static str, StdFunctionRef<'static>)],
    ) {
        for (name, function) in descriptors {
            self.functions.insert(
                SmolStr::new(name),
                StdFunction {
                    params: function.params.clone(),
                    func: function.func,
                },
            );
        }
    }

    /// Register a standard function by name.
    pub fn register(&mut self, name: impl Into<SmolStr>, params: &[&str], func: StdFunc) {
        let params = params
            .iter()
            .map(|param| SmolStr::new(param.to_ascii_uppercase()))
            .collect();
        self.functions.insert(
            SmolStr::new(name.into().as_str().to_ascii_uppercase()),
            StdFunction {
                params: StdParams::Fixed(Cow::Owned(params)),
                func,
            },
        );
    }

    /// Register a standard function with variadic parameters.
    pub fn register_variadic(
        &mut self,
        name: impl Into<SmolStr>,
        prefix: &str,
        start: usize,
        min: usize,
        func: StdFunc,
    ) {
        self.register_variadic_with_fixed(name, &[], prefix, start, min, func);
    }

    /// Register a standard function with fixed and variadic parameters.
    pub fn register_variadic_with_fixed(
        &mut self,
        name: impl Into<SmolStr>,
        fixed: &[&str],
        prefix: &str,
        start: usize,
        min: usize,
        func: StdFunc,
    ) {
        let fixed = fixed
            .iter()
            .map(|param| SmolStr::new(param.to_ascii_uppercase()))
            .collect();
        let prefix = SmolStr::new(prefix.to_ascii_uppercase());
        self.functions.insert(
            SmolStr::new(name.into().as_str().to_ascii_uppercase()),
            StdFunction {
                params: StdParams::Variadic {
                    fixed: Cow::Owned(fixed),
                    prefix,
                    start,
                    min,
                },
                func,
            },
        );
    }

    /// Get borrowed metadata by ASCII-insensitive name without copying parameters.
    ///
    /// ```
    /// use trust_runtime_core::stdlib::StandardLibrary;
    /// use trust_runtime_core::value::Value;
    /// let library = StandardLibrary::new();
    /// let abs = library.get("aBs").unwrap();
    /// assert_eq!((abs.func)(&[Value::Int(-7)]).unwrap(), Value::Int(7));
    /// ```
    #[must_use]
    pub fn get(&self, name: &str) -> Option<StdFunctionRef<'_>> {
        // Avoid allocating a normalized key on the default-only MCU path.
        if !self.functions.is_empty() {
            let key = SmolStr::new(name.to_ascii_uppercase());
            if let Some(function) = self.functions.get(&key) {
                return Some(StdFunctionRef {
                    params: &function.params,
                    func: function.func,
                });
            }
        }
        if self.defaults {
            registration::get(name).copied()
        } else {
            None
        }
    }

    /// Call a standard function by name.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        if let Some(entry) = self.get(name) {
            return (entry.func)(args);
        }
        let key = SmolStr::new(name.to_ascii_uppercase());
        if let Some(result) = conversions::call_conversion(&key, args) {
            return result;
        }
        Err(RuntimeError::UndefinedFunction(name.into()))
    }
}
