//! Which members of the program's types hold an instance of their own (specification 40, §5
//! item 1). A member declared as an interface, a reference or a pointer, or as an array of them,
//! and a `VAR_IN_OUT` or `VAR_EXTERNAL` member only refer to an instance declared elsewhere (IEC
//! 61131-3 Ed.3 §6.6.6 for interfaces, §6.5.2.2 for in-out and external variables). A walk that
//! follows only held instances finds each instance once, where it is declared: a transmitter's
//! alarms appear under the transmitter, not again under every loop and tank it is connected to.

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};

use crate::hmi::member_paths::find_ignoring_case;
use crate::program_model::{ClassDef, FunctionBlockDef, VarDef};
use crate::task::ProgramDef;

/// A chain of type names longer than this is a loop in a malformed program.
const MAX_CHAIN: usize = 64;

/// The program's declarations, for telling held instances from references.
#[derive(Clone, Copy)]
pub(crate) struct Holdings<'a> {
    function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
    classes: &'a IndexMap<SmolStr, ClassDef>,
    programs: &'a IndexMap<SmolStr, ProgramDef>,
    registry: &'a TypeRegistry,
}

/// How a member is declared, as far as holding goes.
enum Declared {
    /// In a section that refers to another's variable (`VAR_IN_OUT`, `VAR_EXTERNAL`).
    Refers,
    Typed(TypeId),
}

impl<'a> Holdings<'a> {
    pub(crate) fn new(
        function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
        classes: &'a IndexMap<SmolStr, ClassDef>,
        programs: &'a IndexMap<SmolStr, ProgramDef>,
        registry: &'a TypeRegistry,
    ) -> Self {
        Self {
            function_blocks,
            classes,
            programs,
            registry,
        }
    }

    /// Whether the member `member` of an instance of `type_name` holds the instance it contains;
    /// a member the declarations do not name is taken to hold it.
    pub(crate) fn holds(&self, type_name: &str, member: &str) -> bool {
        match self.declared(type_name, member) {
            Some(Declared::Refers) => false,
            Some(Declared::Typed(type_id)) => !self.refers(type_id),
            None => true,
        }
    }

    /// A member as one type declares it: a variable, or a function block's input or output.
    fn declared(&self, type_name: &str, member: &str) -> Option<Declared> {
        let variable = |vars: &[VarDef]| {
            vars.iter()
                .find(|var| var.name.eq_ignore_ascii_case(member))
                .map(|var| {
                    if var.in_out || var.external {
                        Declared::Refers
                    } else {
                        Declared::Typed(var.type_id)
                    }
                })
        };
        if let Some(block) = find_ignoring_case(self.function_blocks, type_name) {
            return variable(&block.vars).or_else(|| {
                block
                    .params
                    .iter()
                    .find(|param| param.name.eq_ignore_ascii_case(member))
                    .map(|param| {
                        if matches!(param.direction, ParamDirection::InOut) {
                            Declared::Refers
                        } else {
                            Declared::Typed(param.type_id)
                        }
                    })
            });
        }
        if let Some(class) = find_ignoring_case(self.classes, type_name) {
            return variable(&class.vars);
        }
        find_ignoring_case(self.programs, type_name).and_then(|program| variable(&program.vars))
    }

    /// Whether values of a type refer to an instance rather than hold one: an interface, a
    /// reference or a pointer, through names given to types and arrays of them.
    fn refers(&self, type_id: TypeId) -> bool {
        let mut current = type_id;
        for _ in 0..MAX_CHAIN {
            match self.registry.get(current) {
                Some(Type::Interface { .. } | Type::Reference { .. } | Type::Pointer { .. }) => {
                    return true
                }
                Some(Type::Alias { target, .. }) => current = *target,
                Some(Type::Array { element, .. }) => current = *element,
                _ => return false,
            }
        }
        false
    }
}
