//! Printer for the programs `st_program_gen.rs` generates: the typed AST to Structured Text,
//! with the layout options the metamorphic tests vary.

use super::*;

fn element_to_st(index: &ElemIndex, options: PrintOptions) -> String {
    match index {
        ElemIndex::Const(position) => format!("arr[{position}]"),
        ElemIndex::Dynamic(position) => format!(
            "arr[({} MOD DINT#{ARRAY_LEN} + DINT#{ARRAY_LEN}) MOD DINT#{ARRAY_LEN}]",
            expr_to_st(position, options)
        ),
    }
}

fn expr_to_st(expr: &Expr, options: PrintOptions) -> String {
    match expr {
        Expr::Lit(literal) => literal.to_st(),
        Expr::Var(name) => name.clone(),
        Expr::Element(index) => element_to_st(index, options),
        Expr::Neg(operand) => format!("-({})", expr_to_st(operand, options)),
        Expr::Not(operand) => format!("NOT ({})", expr_to_st(operand, options)),
        Expr::Abs(operand) => format!("ABS({})", expr_to_st(operand, options)),
        Expr::Sqrt(operand) => format!("SQRT({})", expr_to_st(operand, options)),
        Expr::Binary(op, left, right) => {
            format!(
                "({} {} {})",
                expr_to_st(left, options),
                op.symbol(),
                expr_to_st(right, options)
            )
        }
        Expr::Extremum(which, left, right) => {
            format!(
                "{}({}, {})",
                which.name(),
                expr_to_st(left, options),
                expr_to_st(right, options)
            )
        }
        Expr::Limit(low, value, high) => format!(
            "LIMIT({}, {}, {})",
            expr_to_st(low, options),
            expr_to_st(value, options),
            expr_to_st(high, options)
        ),
        Expr::Sel(select, first, second) => format!(
            "SEL({}, {}, {})",
            expr_to_st(select, options),
            expr_to_st(first, options),
            expr_to_st(second, options)
        ),
        Expr::Convert { from, to, arg } => {
            format!(
                "{}_TO_{}({})",
                from.name(),
                to.name(),
                expr_to_st(arg, options)
            )
        }
        Expr::Call { function, a, b } => {
            if options.positional_arguments {
                format!(
                    "{function}({}, {})",
                    expr_to_st(a, options),
                    expr_to_st(b, options)
                )
            } else {
                format!(
                    "{function}(a := {}, b := {})",
                    expr_to_st(a, options),
                    expr_to_st(b, options)
                )
            }
        }
    }
}

fn push_line(out: &mut String, indent: usize, text: &str) {
    out.push_str(&"    ".repeat(indent));
    out.push_str(text);
    out.push('\n');
}

fn statements_to_st(statements: &[Stmt], indent: usize, out: &mut String, options: PrintOptions) {
    for statement in statements {
        statement_to_st(statement, indent, out, options);
    }
}

fn case_item_to_st(item: &CaseItem) -> String {
    match item {
        CaseItem::Single(value) => value.to_string(),
        CaseItem::Range(low, high) => format!("{low}..{high}"),
    }
}

fn statement_to_st(statement: &Stmt, indent: usize, out: &mut String, options: PrintOptions) {
    match statement {
        Stmt::Assign { target, value } => {
            push_line(
                out,
                indent,
                &format!("{target} := {};", expr_to_st(value, options)),
            );
        }
        Stmt::AssignElement { index, value } => {
            push_line(
                out,
                indent,
                &format!(
                    "{} := {};",
                    element_to_st(index, options),
                    expr_to_st(value, options)
                ),
            );
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            for (position, (condition, body)) in branches.iter().enumerate() {
                let keyword = if position == 0 { "IF" } else { "ELSIF" };
                push_line(
                    out,
                    indent,
                    &format!("{keyword} {} THEN", expr_to_st(condition, options)),
                );
                statements_to_st(body, indent + 1, out, options);
            }
            if let Some(body) = else_body {
                push_line(out, indent, "ELSE");
                statements_to_st(body, indent + 1, out, options);
            }
            push_line(out, indent, "END_IF;");
        }
        Stmt::Case {
            selector,
            arms,
            else_body,
        } => {
            push_line(
                out,
                indent,
                &format!("CASE {} OF", expr_to_st(selector, options)),
            );
            for (items, body) in arms {
                let labels: Vec<String> = items.iter().map(case_item_to_st).collect();
                push_line(out, indent, &format!("{}:", labels.join(", ")));
                statements_to_st(body, indent + 1, out, options);
            }
            push_line(out, indent, "ELSE");
            statements_to_st(else_body, indent + 1, out, options);
            push_line(out, indent, "END_CASE;");
        }
        Stmt::For { upper, body } => {
            push_line(out, indent, &format!("FOR i := 0 TO {upper} DO"));
            statements_to_st(body, indent + 1, out, options);
            push_line(out, indent, "END_FOR;");
        }
        Stmt::While { bound, body } => {
            push_line(out, indent, "k := DINT#0;");
            push_line(out, indent, &format!("WHILE k < DINT#{bound} DO"));
            push_line(out, indent + 1, "k := k + DINT#1;");
            statements_to_st(body, indent + 1, out, options);
            push_line(out, indent, "END_WHILE;");
        }
    }
}

/// Layout choices that leave the meaning of a program unchanged. `PrintOptions::default()` is the
/// layout the A03 seeds were generated with, so the default output must not change.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrintOptions {
    /// Print each call as `F0(x, y)`, in declaration order, instead of `F0(a := x, b := y)`.
    pub positional_arguments: bool,
    /// Print the `FUNCTION` declarations after `PROGRAM Main` instead of before it.
    pub functions_after_main: bool,
    /// Print the `FUNCTION` declarations last declared first.
    pub reverse_functions: bool,
}

/// Prints the program with the default options.
pub(super) fn render(program: &ProgramAst) -> String {
    render_with(program, PrintOptions::default())
}

/// Prints the program. Its declarations are fixed; the state variables, functions and body
/// come from the AST.
pub(super) fn render_with(program: &ProgramAst, options: PrintOptions) -> String {
    let mut out = String::from(
        "VAR_GLOBAL\n    in_d0 : DINT := DINT#0;\n    in_d1 : DINT := DINT#0;\n    in_b0 : BOOL := FALSE;\n    in_r0 : REAL := REAL#0.0;\nEND_VAR\n",
    );
    if !options.functions_after_main {
        functions_to_st(program, &mut out, options);
    }
    out.push_str("PROGRAM Main\nVAR\n");
    for variable in &program.state {
        out.push_str(&format!(
            "    {} : {} := {};\n",
            variable.name,
            variable.ty.name(),
            variable.init.to_st()
        ));
    }
    out.push_str(&format!(
        "    arr : ARRAY[0..{}] OF DINT;\n    i : DINT;\n    k : DINT;\n    cycles : DINT;\nEND_VAR\ncycles := cycles + DINT#1;\n",
        ARRAY_LEN - 1
    ));
    statements_to_st(&program.body, 0, &mut out, options);
    out.push_str("END_PROGRAM\n");
    if options.functions_after_main {
        functions_to_st(program, &mut out, options);
    }
    out
}

/// Prints the `FUNCTION` declarations in declaration order, or last declared first when the
/// options reverse them.
fn functions_to_st(program: &ProgramAst, out: &mut String, options: PrintOptions) {
    let mut functions: Vec<&FunctionDecl> = program.functions.iter().collect();
    if options.reverse_functions {
        functions.reverse();
    }
    for function in functions {
        out.push_str(&format!(
            "FUNCTION {name} : DINT\nVAR_INPUT\n    a : DINT;\n    b : DINT;\nEND_VAR\n{name} := {body};\nEND_FUNCTION\n",
            name = function.name,
            body = expr_to_st(&function.body, options),
        ));
    }
}
