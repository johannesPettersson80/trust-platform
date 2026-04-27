mod common;
mod errors;
mod expr_access;
mod expr_bool;
mod expr_coercion;
mod expr_full;
mod expr_literals;
mod expr_ops;
mod expr_time_ops;
mod pou_en_eno;
mod pou_fb;
mod pou_function;
mod pou_params;
mod reference;
mod stmt_basic;
mod stmt_case;
mod stmt_if;
mod stmt_loops;
mod stmt_return;

#[test]
fn debug_hook_fires_once_per_statement() {
    use trust_hir::types::TypeRegistry;

    use crate::debug::{DebugHook, SourceLocation};
    use crate::eval::stmt::{exec_stmt, Stmt};
    use crate::eval::EvalContext;
    use crate::memory::VariableStorage;
    use crate::value::{DateTimeProfile, Duration, Value};

    struct CountingHook {
        count: usize,
    }

    impl DebugHook for CountingHook {
        fn on_statement(&mut self, _location: Option<&SourceLocation>, _call_depth: u32) {
            self.count += 1;
        }
    }

    let mut storage = VariableStorage::default();
    storage.push_frame("MAIN");
    let registry = TypeRegistry::new();
    let mut hook = CountingHook { count: 0 };
    let mut ctx = EvalContext {
        storage: &mut storage,
        registry: &registry,
        initializer_catalog: None,
        profile: DateTimeProfile::default(),
        now: Duration::ZERO,
        debug: Some(&mut hook),
        call_depth: 0,
        functions: None,
        stdlib: None,
        function_blocks: None,
        classes: None,
        using: None,
        access: None,
        current_instance: None,
        return_name: None,
        loop_depth: 0,
        execution_deadline: None,
    };

    let stmt = Stmt::Expr {
        expr: crate::program_model::Expr::Literal(Value::Int(1)),
        location: None,
    };

    let _ = exec_stmt(&mut ctx, &stmt).unwrap();

    let expected = if cfg!(feature = "debug") { 1 } else { 0 };
    assert_eq!(hook.count, expected);
}
