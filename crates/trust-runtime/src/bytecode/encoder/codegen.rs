use smol_str::SmolStr;

use crate::bytecode::DebugEntry;
use crate::value::{Value, ValueRef};

use super::consts::type_id_for_value;
use super::util::to_u32;
use super::{AccessKind, BytecodeEncoder, BytecodeError, CodegenContext};

include!("codegen/dynamic_access.rs");
include!("codegen/expr.rs");
include!("codegen/call_expr.rs");
include!("codegen/reference_attempt.rs");
include!("codegen/stmt_core.rs");
include!("codegen/stmt_branches.rs");
include!("codegen/stmt_loops.rs");
include!("codegen/jumps_consts.rs");
include!("codegen/expr_supported.rs");

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::memory::MemoryLocation;
    use crate::program_model::{ArgValue, CallArg, Expr, LValue, SizeOfTarget, Stmt};
    use trust_hir::TypeId;

    #[test]
    fn lowering_support_predicates_cover_nested_call_sizeof_and_lvalue_shapes() {
        let indexed_target = LValue::Index {
            target: Box::new(LValue::Name(SmolStr::new("items"))),
            indices: vec![Expr::Literal(Value::Int(1))],
        };
        let target_argument = CallArg {
            name: Some(SmolStr::new("value")),
            value: ArgValue::Target(indexed_target.clone()),
        };
        let call = Expr::Call {
            target: Box::new(Expr::Name(SmolStr::new("Transform"))),
            args: vec![target_argument.clone()],
        };
        let call_lvalue = LValue::Deref(Box::new(call.clone()));

        assert!(call_arg_supported(&target_argument));
        assert!(lvalue_supported(&indexed_target));
        assert!(expr_supported(&call));
        assert!(lvalue_contains_call(&call_lvalue));
        assert!(expr_contains_call(&Expr::Ref(call_lvalue.clone())));
        assert!(stmt_contains_c1_required_call(&Stmt::Assign {
            target: LValue::Name(SmolStr::new("result")),
            value: Expr::Ref(call_lvalue),
            location: None,
        }));

        let sizeof = Expr::SizeOf(SizeOfTarget::Type(TypeId::INT));
        let sizeof_lvalue = LValue::Deref(Box::new(sizeof));
        assert!(lvalue_contains_sizeof(&sizeof_lvalue));
        assert!(expr_contains_sizeof(&Expr::Ref(sizeof_lvalue.clone())));
        assert!(stmt_contains_c5_required_construct(&Stmt::Return {
            expr: Some(Expr::Ref(sizeof_lvalue)),
            location: None,
        }));

        assert!(!expr_contains_call(&Expr::Name(SmolStr::new("plain"))));
        assert!(!lvalue_contains_sizeof(&LValue::Name(SmolStr::new(
            "plain"
        ))));
    }

    #[test]
    fn return_and_ref_builtin_lowering_preserve_exact_fail_closed_boundaries() {
        let runtime = crate::Runtime::new();
        let mut encoder = BytecodeEncoder::new(&runtime);
        let mut context = CodegenContext::default();
        let mut code = Vec::new();

        assert!(encoder
            .emit_return_stmt(&mut context, None, &mut code)
            .expect("bare return must lower"));
        assert_eq!(code, vec![0x06]);
        assert!(!encoder
            .emit_return_stmt(&mut context, Some(&Expr::Literal(Value::Int(1))), &mut code,)
            .expect("value return without a return slot is unsupported"));

        let no_args = encoder
            .emit_ref_builtin_call(&context, &[], &mut Vec::new())
            .expect_err("REF without exactly one argument must fail");
        assert!(no_args.to_string().contains("exactly one argument"));

        let local = ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: Vec::new(),
        };
        context = CodegenContext::new(
            None,
            None,
            Vec::new(),
            HashMap::from([(SmolStr::new("VALUE"), local)]),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );
        let args = [CallArg {
            name: None,
            value: ArgValue::Target(LValue::Name(SmolStr::new("value"))),
        }];
        let mut ref_code = Vec::new();
        assert!(encoder
            .emit_ref_builtin_call(&context, &args, &mut ref_code)
            .expect("one addressable target must lower"));
        assert_eq!(ref_code.first(), Some(&0x22));
        assert_eq!(ref_code.len(), 5);
    }

    #[test]
    fn partial_access_lowering_emits_exact_opcode_and_typed_operand() {
        let runtime = crate::Runtime::new();
        let mut encoder = BytecodeEncoder::new(&runtime);
        let local = ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: Vec::new(),
        };
        let context = CodegenContext::new(
            None,
            None,
            Vec::new(),
            HashMap::from([(SmolStr::new("VALUE"), local)]),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );

        let mut read_code = Vec::new();
        assert!(encoder
            .emit_partial_read_for_name(
                &context,
                &SmolStr::new("value"),
                crate::value::PartialAccess::Byte(2),
                &mut read_code,
            )
            .expect("a static partial read must lower"));
        assert_eq!(
            &read_code[read_code.len() - 5..],
            &[0x62, 0x02, 0x01, 0x00, 0x00]
        );

        let mut write_code = Vec::new();
        encoder.emit_partial_write(crate::value::PartialAccess::Word(3), &mut write_code);
        assert_eq!(write_code, [0x63, 0x03, 0x02, 0x00, 0x00]);
    }
}
