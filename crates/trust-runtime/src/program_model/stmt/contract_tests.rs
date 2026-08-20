use super::*;

fn location() -> SourceLocation {
    SourceLocation::new(7, 11, 19)
}

fn expression(value: i16) -> Expr {
    Expr::Literal(Value::Int(value))
}

fn target(name: &str) -> LValue {
    LValue::Name(name.into())
}

fn statements(location: Option<SourceLocation>) -> Vec<Stmt> {
    vec![
        Stmt::Assign {
            target: target("assign"),
            value: expression(1),
            location,
        },
        Stmt::AssignAttempt {
            target: target("attempt"),
            value: expression(2),
            target_type: trust_hir::TypeId::INT,
            location,
        },
        Stmt::Expr {
            expr: expression(3),
            location,
        },
        Stmt::If {
            condition: Expr::Literal(Value::Bool(true)),
            then_block: vec![],
            else_if: vec![],
            else_block: vec![],
            location,
        },
        Stmt::Case {
            selector: expression(4),
            branches: vec![],
            else_block: vec![],
            location,
        },
        Stmt::For {
            control: "i".into(),
            start: expression(1),
            end: expression(3),
            step: expression(1),
            body: vec![],
            location,
        },
        Stmt::While {
            condition: Expr::Literal(Value::Bool(true)),
            body: vec![],
            location,
        },
        Stmt::Repeat {
            body: vec![],
            until: Expr::Literal(Value::Bool(true)),
            location,
        },
        Stmt::Label {
            name: "again".into(),
            stmt: None,
            location,
        },
        Stmt::Jmp {
            target: "again".into(),
            location,
        },
        Stmt::Return {
            expr: Some(expression(5)),
            location,
        },
        Stmt::Exit { location },
        Stmt::Continue { location },
    ]
}

#[test]
fn every_statement_variant_returns_its_exact_source_location() {
    let expected = location();
    let statements = statements(Some(expected));

    assert_eq!(statements.len(), 13);
    for statement in &statements {
        assert_eq!(statement.location(), Some(&expected));
    }
}

#[test]
fn every_statement_variant_preserves_absent_source_location() {
    for statement in statements(None) {
        assert_eq!(statement.location(), None);
    }
}

#[test]
fn statement_result_variants_preserve_control_flow_payloads_when_cloned() {
    let results = [
        StmtResult::Continue,
        StmtResult::Return(None),
        StmtResult::Return(Some(Value::DInt(42))),
        StmtResult::Exit,
        StmtResult::LoopContinue,
        StmtResult::Jump("target".into()),
    ];

    for result in results {
        assert_eq!(result.clone(), result);
    }
}

#[test]
fn case_labels_preserve_single_values_and_inclusive_range_endpoints() {
    let single = CaseLabel::Single(Value::String("Mode".into()));
    let range = CaseLabel::Range(-3, 7);

    assert!(matches!(
        single.clone(),
        CaseLabel::Single(Value::String(value)) if value == "Mode"
    ));
    assert!(matches!(range.clone(), CaseLabel::Range(-3, 7)));
}

#[test]
fn if_statement_preserves_ordered_elsif_and_separate_else_blocks() {
    let statement = Stmt::If {
        condition: Expr::Name("first".into()),
        then_block: vec![Stmt::Exit { location: None }],
        else_if: vec![
            (
                Expr::Name("second".into()),
                vec![Stmt::Continue { location: None }],
            ),
            (
                Expr::Name("third".into()),
                vec![Stmt::Return {
                    expr: None,
                    location: None,
                }],
            ),
        ],
        else_block: vec![Stmt::Expr {
            expr: expression(9),
            location: None,
        }],
        location: Some(location()),
    };

    let Stmt::If {
        then_block,
        else_if,
        else_block,
        ..
    } = statement.clone()
    else {
        panic!("expected IF");
    };
    assert_eq!(then_block.len(), 1);
    assert_eq!(else_if.len(), 2);
    assert!(matches!(&else_if[0].0, Expr::Name(name) if name == "second"));
    assert!(matches!(&else_if[1].0, Expr::Name(name) if name == "third"));
    assert_eq!(else_block.len(), 1);
}

#[test]
fn case_statement_preserves_branch_and_label_order_without_normalizing_ranges() {
    let statement = Stmt::Case {
        selector: Expr::Name("selector".into()),
        branches: vec![
            (
                vec![CaseLabel::Single(Value::Int(5)), CaseLabel::Range(9, 3)],
                vec![Stmt::Exit { location: None }],
            ),
            (
                vec![CaseLabel::Single(Value::Int(1))],
                vec![Stmt::Continue { location: None }],
            ),
        ],
        else_block: vec![Stmt::Return {
            expr: None,
            location: None,
        }],
        location: Some(location()),
    };

    let Stmt::Case {
        branches,
        else_block,
        ..
    } = statement.clone()
    else {
        panic!("expected CASE");
    };
    assert_eq!(branches.len(), 2);
    assert!(matches!(
        &branches[0].0[0],
        CaseLabel::Single(Value::Int(5))
    ));
    assert!(matches!(&branches[0].0[1], CaseLabel::Range(9, 3)));
    assert!(matches!(
        &branches[1].0[0],
        CaseLabel::Single(Value::Int(1))
    ));
    assert_eq!(else_block.len(), 1);
}

#[test]
fn for_statement_preserves_control_bounds_step_and_body_order() {
    let statement = Stmt::For {
        control: "index".into(),
        start: expression(-2),
        end: expression(8),
        step: expression(2),
        body: vec![
            Stmt::Continue { location: None },
            Stmt::Exit { location: None },
        ],
        location: Some(location()),
    };

    let Stmt::For {
        control,
        start,
        end,
        step,
        body,
        ..
    } = statement.clone()
    else {
        panic!("expected FOR");
    };
    assert_eq!(control, "index");
    assert!(matches!(start, Expr::Literal(Value::Int(-2))));
    assert!(matches!(end, Expr::Literal(Value::Int(8))));
    assert!(matches!(step, Expr::Literal(Value::Int(2))));
    assert!(matches!(&body[0], Stmt::Continue { .. }));
    assert!(matches!(&body[1], Stmt::Exit { .. }));
}

#[test]
fn label_and_jump_keep_distinct_names_and_attached_statement() {
    let label = Stmt::Label {
        name: "entry".into(),
        stmt: Some(Box::new(Stmt::Return {
            expr: Some(expression(4)),
            location: None,
        })),
        location: Some(location()),
    };
    let jump = Stmt::Jmp {
        target: "exit".into(),
        location: Some(location()),
    };

    assert!(matches!(
        label.clone(),
        Stmt::Label {
            name,
            stmt: Some(_),
            ..
        } if name == "entry"
    ));
    assert!(matches!(
        jump.clone(),
        Stmt::Jmp { target, .. } if target == "exit"
    ));
}

#[test]
fn assignment_and_reference_assignment_attempt_remain_distinct_nodes() {
    let assignment = Stmt::Assign {
        target: target("x"),
        value: expression(1),
        location: None,
    };
    let attempt = Stmt::AssignAttempt {
        target: target("x"),
        value: expression(1),
        target_type: trust_hir::TypeId::INT,
        location: None,
    };

    assert!(matches!(assignment.clone(), Stmt::Assign { .. }));
    assert!(matches!(attempt.clone(), Stmt::AssignAttempt { .. }));
}
