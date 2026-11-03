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

    pub(in crate::type_check) fn check_formal_arg_count(
        &self,
        checker: &mut StandardChecker<'_, '_>,
        node: &SyntaxNode,
        expected: usize,
    ) {
        checker.check_formal_arg_count(&self.bound, node, self.arg_count, expected);
    }

    pub(in crate::type_check) fn arg(&self, index: usize) -> Option<(CallArg, TypeId)> {
        self.typed.get(index).and_then(|arg| arg.clone())
    }

    pub(in crate::type_check) fn args_from(&self, start: usize) -> Vec<(CallArg, TypeId)> {
        self.typed.iter().skip(start).flatten().cloned().collect()
    }
}

impl<'a, 'b> StandardChecker<'a, 'b> {
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
