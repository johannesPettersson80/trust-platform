//! Standard library registry.

pub mod assertions;
pub mod bit;
pub mod comparison;
pub mod conversions;
pub mod fbs;
pub mod helpers;
pub mod numeric;
pub mod selection;
pub mod string;
pub mod time;
pub mod validate;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::value::Value;

/// Standard function signature.
pub type StdFunc = fn(&[Value]) -> Result<Value, RuntimeError>;

/// Standard function parameter specification.
#[derive(Debug, Clone)]
pub enum StdParams {
    /// Fixed parameter list.
    Fixed(Vec<SmolStr>),
    /// Variadic parameters with a numeric suffix (e.g., IN1, IN2, ...).
    Variadic {
        /// Fixed parameter names that appear before the variadic set.
        fixed: Vec<SmolStr>,
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

/// Standard library registry for functions/FBs.
#[derive(Debug, Default, Clone)]
pub struct StandardLibrary {
    functions: IndexMap<SmolStr, StdFunction>,
}

impl StandardLibrary {
    /// Build a standard library with default functions.
    #[must_use]
    pub fn new() -> Self {
        let mut lib = Self {
            functions: IndexMap::new(),
        };
        assertions::register(&mut lib);
        numeric::register(&mut lib);
        bit::register(&mut lib);
        selection::register(&mut lib);
        comparison::register(&mut lib);
        string::register(&mut lib);
        time::register(&mut lib);
        validate::register(&mut lib);
        conversions::register(&mut lib);
        lib
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
                params: StdParams::Fixed(params),
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
                    fixed,
                    prefix,
                    start,
                    min,
                },
                func,
            },
        );
    }

    /// Get a standard function by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&StdFunction> {
        let key = SmolStr::new(name.to_ascii_uppercase());
        self.functions.get(&key)
    }

    /// Call a standard function by name.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let key = SmolStr::new(name.to_ascii_uppercase());
        if let Some(entry) = self.functions.get(&key) {
            return (entry.func)(args);
        }
        if let Some(result) = conversions::call_conversion(&key, args) {
            return result;
        }
        Err(RuntimeError::UndefinedFunction(name.into()))
    }
}
