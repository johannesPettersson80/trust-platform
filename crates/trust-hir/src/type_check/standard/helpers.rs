use super::super::calls::{BoundArgs, CallArg, ParamInfo};
use super::super::*;

pub(in crate::type_check) fn builtin_param(name: &str, direction: ParamDirection) -> ParamInfo {
    ParamInfo {
        name: SmolStr::new(name),
        type_id: TypeId::ANY,
        direction,
    }
}

pub(in crate::type_check) fn builtin_in_params(
    prefix: &str,
    start: usize,
    count: usize,
) -> Vec<ParamInfo> {
    (0..count)
        .map(|offset| builtin_param(&format!("{}{}", prefix, start + offset), ParamDirection::In))
        .collect()
}

pub(in crate::type_check) fn is_execution_param(param: &ParamInfo) -> bool {
    param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO")
}

pub(in crate::type_check) struct BuiltinCall {
    arg_count: usize,
    bound: BoundArgs,
    typed: Vec<Option<(CallArg, TypeId)>>,
}

impl BuiltinCall {
    pub(in crate::type_check) fn arg_count(&self) -> usize {
        self.arg_count
    }

    /// Refuses a formal call that does not give the function `expected` arguments. It counts the
    /// arguments bound to the function's parameters: a name the function does not have (`EN`
    /// among them, specification 04) or one given twice is refused on its own, and counting it
    /// again gave a second, wrong count.
    pub(in crate::type_check) fn check_formal_arg_count(
        &self,
        checker: &mut StandardChecker<'_, '_>,
        node: &SyntaxNode,
        expected: usize,
    ) {
        let bound = self.bound.assigned.iter().flatten().count();
        checker.check_formal_arg_count(&self.bound, node, bound, expected);
    }

    pub(in crate::type_check) fn arg(&self, index: usize) -> Option<(CallArg, TypeId)> {
        self.typed.get(index).and_then(|arg| arg.clone())
    }

    /// Whether the call gives the function's `count` inputs as IEC 61131-3 Ed.3 §6.6.1.4.2 allows:
    /// a non-formal call each of them in order, a formal call any of them, as "the formal parameter
    /// list may be complete or incomplete" and an input it omits takes the default value of its
    /// type (specification 07 §1). The binding refuses a non-formal call's wrong count, and a formal
    /// call's unknown and repeated names, on their own.
    pub(in crate::type_check) fn gives_inputs(&self, count: usize) -> bool {
        self.bound.formal_call || self.arg_count == count
    }

    pub(in crate::type_check) fn args_from(&self, start: usize) -> Vec<(CallArg, TypeId)> {
        self.typed.iter().skip(start).flatten().cloned().collect()
    }
}

impl<'a, 'b> StandardChecker<'a, 'b> {
    /// Refuses a formal call of standard function `function` that omits `inputs`, which give the
    /// function its type: an input a formal call omits takes the default value of that type (IEC
    /// 61131-3 Ed.3 §6.6.1.4.2; specification 07 §1), and nothing gives it.
    pub(in crate::type_check) fn untyped_call(
        &mut self,
        node: &SyntaxNode,
        function: &str,
        inputs: &str,
    ) -> TypeId {
        self.checker.legacy_diagnostic_type(
            DiagnosticCode::WrongArgumentCount,
            node.text_range(),
            format!(
                "{function} needs {inputs} to give its type: an input that a formal call omits takes \
                 the default value of that type"
            ),
        )
    }

    /// Refuses a formal call of `SUB` or `DIV` of a time that omits an input: the type of the
    /// omitted input depends on the overload the other input picks (IEC 61131-3 Ed.3 Table 37), so
    /// it has no default value to take (specification 07 §1).
    pub(in crate::type_check) fn both_time_inputs(
        &mut self,
        node: &SyntaxNode,
        function: &str,
    ) -> TypeId {
        self.checker.legacy_diagnostic_type(
            DiagnosticCode::WrongArgumentCount,
            node.text_range(),
            format!(
                "{function} of a time needs IN1 and IN2: the type of an omitted input depends on \
                 the other's (IEC 61131-3 Ed.3 Table 37)"
            ),
        )
    }

    pub(in crate::type_check) fn builtin_call(
        &mut self,
        node: &SyntaxNode,
        params: Vec<ParamInfo>,
    ) -> BuiltinCall {
        let arg_count = self.checker.calls().collect_call_args(node).len();
        // Standard functions keep the full positional association, outputs included (e.g.
        // SPLIT_DATE), whatever the project's profile.
        let (bound, typed) = self
            .checker
            .calls()
            .collect_builtin_args(&params, node, true);
        BuiltinCall {
            arg_count,
            bound,
            typed,
        }
    }
}
