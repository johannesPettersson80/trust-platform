fn expr_supported(expr: &crate::program_model::Expr) -> bool {
    use crate::program_model::Expr;
    use crate::program_model::{BinaryOp, UnaryOp};
    match expr {
        Expr::Literal(value) => {
            type_id_for_value(value).is_some() || matches!(value, crate::value::Value::Null)
        }
        Expr::ArrayInitializer(_) => false,
        Expr::StructInitializer(_) => false,
        Expr::Name(_) => true,
        Expr::This | Expr::Super => true,
        Expr::SizeOf(crate::program_model::SizeOfTarget::Type(_)) => true,
        Expr::Field { target, field: _ } => expr_supported(target),
        Expr::Index { target, indices } => {
            expr_supported(target) && indices.iter().all(expr_supported)
        }
        Expr::Ref(target) => lvalue_supported(target),
        Expr::Deref(expr) => expr_supported(expr),
        Expr::Unary { op, expr } => {
            matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::Pos) && expr_supported(expr)
        }
        Expr::Binary { op, left, right } => {
            matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Pow
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
            ) && expr_supported(left)
                && expr_supported(right)
        }
        Expr::Call { target, args } => {
            matches!(
                target.as_ref(),
                Expr::Name(_) | Expr::Field { .. }
            ) && args.iter().all(call_arg_supported)
        }
    }
}

fn call_arg_supported(arg: &crate::program_model::CallArg) -> bool {
    use crate::program_model::ArgValue;
    match &arg.value {
        ArgValue::Expr(expr) => expr_supported(expr),
        ArgValue::Target(target) => lvalue_supported(target),
    }
}

fn lvalue_supported(target: &crate::program_model::LValue) -> bool {
    match target {
        crate::program_model::LValue::Name(_) => true,
        crate::program_model::LValue::Field { target, .. } => lvalue_supported(target),
        crate::program_model::LValue::Index { target, indices } => {
            lvalue_supported(target) && indices.iter().all(expr_supported)
        }
        crate::program_model::LValue::Deref(expr) => expr_supported(expr),
    }
}
