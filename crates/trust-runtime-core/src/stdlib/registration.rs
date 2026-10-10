//! One registration list feeds both allocation-free accounting and construction.

use super::{StandardLibrary, StdFunc, StdFunction};
use crate::error::RuntimeError;
use smol_str::SmolStr;

pub(super) trait Registration {
    fn register(&mut self, name: &str, params: &[&str], func: StdFunc);
    fn register_variadic_with_fixed(
        &mut self,
        name: &str,
        fixed: &[&str],
        prefix: &str,
        start: usize,
        min: usize,
        func: StdFunc,
    );
    fn register_variadic(
        &mut self,
        name: &str,
        prefix: &str,
        start: usize,
        min: usize,
        func: StdFunc,
    ) {
        self.register_variadic_with_fixed(name, &[], prefix, start, min, func);
    }
}

impl Registration for StandardLibrary {
    fn register(&mut self, name: &str, params: &[&str], func: StdFunc) {
        StandardLibrary::register(self, name, params, func);
    }
    fn register_variadic_with_fixed(
        &mut self,
        name: &str,
        fixed: &[&str],
        prefix: &str,
        start: usize,
        min: usize,
        func: StdFunc,
    ) {
        StandardLibrary::register_variadic_with_fixed(self, name, fixed, prefix, start, min, func);
    }
}

#[derive(Default)]
struct Demand {
    bytes: usize,
    work: usize,
    overflow: bool,
}

impl Demand {
    fn add(&mut self, name: &str, params: &[&str], prefix: Option<&str>) {
        let Some(text) = core::iter::once(name)
            .chain(prefix)
            .chain(params.iter().copied())
            .try_fold(0usize, |sum, text| sum.checked_add(text.len()))
        else {
            self.overflow = true;
            return;
        };
        let bytes = params
            .len()
            .checked_mul(core::mem::size_of::<SmolStr>())
            .and_then(|bytes| {
                bytes.checked_add(
                    core::mem::size_of::<(SmolStr, StdFunction)>()
                        + 4 * core::mem::size_of::<usize>(),
                )
            })
            .and_then(|bytes| bytes.checked_add(text))
            .and_then(|bytes| self.bytes.checked_add(bytes));
        let work = text
            .checked_add(1)
            .and_then(|work| self.work.checked_add(work));
        match (bytes, work) {
            (Some(bytes), Some(work)) => {
                self.bytes = bytes;
                self.work = work;
            }
            _ => self.overflow = true,
        }
    }
}

impl Registration for Demand {
    fn register(&mut self, name: &str, params: &[&str], _func: StdFunc) {
        self.add(name, params, None);
    }
    fn register_variadic_with_fixed(
        &mut self,
        name: &str,
        fixed: &[&str],
        prefix: &str,
        _start: usize,
        _min: usize,
        _func: StdFunc,
    ) {
        self.add(name, fixed, Some(prefix));
    }
}

pub(super) fn preparation_demand() -> Result<(usize, usize), RuntimeError> {
    let mut demand = Demand::default();
    register_defaults(&mut demand);
    if demand.overflow {
        return Err(RuntimeError::Overflow);
    }
    Ok((demand.bytes, demand.work))
}

pub(super) fn register_defaults(lib: &mut impl Registration) {
    super::assertions::register_into(lib);
    super::numeric::register_into(lib);
    super::bit::register_into(lib);
    super::selection::register_into(lib);
    super::comparison::register_into(lib);
    super::string::register_into(lib);
    super::time::register_into(lib);
    super::validate::register_into(lib);
    // Conversion recognition is algorithmic and registers no stored entries.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_free_demand_matches_the_constructed_registry() {
        let library = StandardLibrary::new();
        let mut bytes = library.functions.len()
            * (core::mem::size_of::<(SmolStr, StdFunction)>() + 4 * core::mem::size_of::<usize>());
        let mut work = library.functions.len();
        for (name, function) in &library.functions {
            let (params, prefix) = match &function.params {
                super::super::StdParams::Fixed(params) => (params, None),
                super::super::StdParams::Variadic { fixed, prefix, .. } => (fixed, Some(prefix)),
            };
            bytes += params.len() * core::mem::size_of::<SmolStr>();
            for text in core::iter::once(name).chain(prefix).chain(params.iter()) {
                bytes += text.len();
                work += text.len();
            }
        }
        assert_eq!(preparation_demand().unwrap(), (bytes, work));
    }
}
