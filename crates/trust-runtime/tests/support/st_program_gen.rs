//! Bounded generator of Structured Text programs for `property_executor_differential.rs` and
//! `property_metamorphic.rs`.
//!
//! A program is a typed AST built with proptest strategies and printed by `render`. It stays
//! inside the forms the truST compiler accepts, as the specifications describe:
//!
//! - six elementary types with typed literals such as `DINT#5`, `INT#-3`, `REAL#-1.75`
//!   (docs/specs/01-lexical-elements.md sections 6A and 7);
//! - every operator and comparison takes two operands of one identical type, so no implicit
//!   conversion is used; conversions are explicit `<FROM>_TO_<TO>` calls
//!   (docs/specs/02-data-types.md section 5, docs/specs/05-expressions.md section 6.1);
//! - `CASE` labels are distinct integers and ranges with an `ELSE` arm; `FOR` has no `BY`
//!   clause and never assigns its control variable in the body; `WHILE` is driven by `k`, which
//!   only its own loop assigns, and loops of the same kind never nest
//!   (docs/specs/06-statements.md sections 6 to 8);
//! - `ABS`, `SQRT`, `MIN`, `MAX`, `LIMIT`, `SEL` and the `<FROM>_TO_<TO>` conversions as in
//!   docs/specs/07-standard-functions.md sections 2, 3 and 6;
//! - `arr[index]` is always in range: a constant or `(e MOD 8 + 8) MOD 8`.
//!
//! Division by zero and integer overflow are generated on purpose. The differential test only
//! requires the executors to agree on the value or on the fault.

#![allow(dead_code)]

use proptest::prelude::*;
use proptest::strategy::{BoxedStrategy, Strategy, Union};
use std::collections::BTreeSet;
use std::fmt;
use trust_runtime_core::value::Value;

#[path = "st_program_print.rs"]
mod print;
pub use print::PrintOptions;
use print::{render, render_with};

/// Input globals written before every cycle, in the order of an input row.
pub const INPUT_NAMES: [&str; 4] = ["in_d0", "in_d1", "in_b0", "in_r0"];

/// Cycles per program, which is also the length of every input table.
const CYCLES: usize = 6;
/// Deepest typed expression; a literal or variable has depth 0.
const MAX_EXPR_DEPTH: usize = 4;
/// Deepest statement nesting; top-level statements have depth 1.
const MAX_STMT_DEPTH: u32 = 3;
/// Length of `arr : ARRAY[0..7] OF DINT`.
const ARRAY_LEN: i64 = 8;
/// Printed programs stay under about this many lines.
const MAX_LINES: usize = 200;
/// `CASE` labels are distinct values in `0..=CASE_LABEL_MAX`.
const CASE_LABEL_MAX: i64 = 7;
/// Comparison operators, chosen uniformly.
const COMPARISONS: [BinOp; 6] = [
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Le,
    BinOp::Gt,
    BinOp::Ge,
];

/// The elementary types of the generated programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Bool,
    Int,
    DInt,
    LInt,
    Real,
    LReal,
}

impl Ty {
    const ALL: [Ty; 6] = [Ty::Bool, Ty::Int, Ty::DInt, Ty::LInt, Ty::Real, Ty::LReal];
    const NUMERIC: [Ty; 5] = [Ty::Int, Ty::DInt, Ty::LInt, Ty::Real, Ty::LReal];

    /// Position in `ALL`, used to index the expression tables.
    fn index(self) -> usize {
        match self {
            Ty::Bool => 0,
            Ty::Int => 1,
            Ty::DInt => 2,
            Ty::LInt => 3,
            Ty::Real => 4,
            Ty::LReal => 5,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Ty::Bool => "BOOL",
            Ty::Int => "INT",
            Ty::DInt => "DINT",
            Ty::LInt => "LINT",
            Ty::Real => "REAL",
            Ty::LReal => "LREAL",
        }
    }

    /// Inclusive range of generated integer literals. Only the integer types use it.
    fn integer_bounds(self) -> (i64, i64) {
        match self {
            Ty::Int => (-100, 100),
            Ty::DInt => (-1000, 1000),
            _ => (-100_000, 100_000),
        }
    }
}

/// A typed literal, printed with its type prefix.
#[derive(Debug, Clone, Copy)]
pub enum Literal {
    Bool(bool),
    Int {
        ty: Ty,
        value: i64,
    },
    /// A real value in quarter units: `REAL#1.75` is `Real { quarters: 7, .. }`. Quarters are
    /// exact in binary and print without rounding.
    Real {
        ty: Ty,
        quarters: i32,
    },
}

impl Literal {
    fn to_st(self) -> String {
        match self {
            Literal::Bool(value) => String::from(if value { "TRUE" } else { "FALSE" }),
            Literal::Int { ty, value } => format!("{}#{value}", ty.name()),
            Literal::Real { ty, quarters } => format!("{}#{}", ty.name(), real_text(quarters)),
        }
    }
}

/// Decimal text of `quarters / 4`, exact because the value is a multiple of 0.25.
fn real_text(quarters: i32) -> String {
    let sign = if quarters < 0 { "-" } else { "" };
    let magnitude = quarters.unsigned_abs();
    format!("{sign}{}.{:02}", magnitude / 4, (magnitude % 4) * 25)
}

/// Binary operators and comparisons. Both operands have the same type.
#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "MOD",
            BinOp::And => "AND",
            BinOp::Or => "OR",
            BinOp::Xor => "XOR",
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }
}

/// `MIN` or `MAX` of two operands.
#[derive(Debug, Clone, Copy)]
pub enum Extremum {
    Min,
    Max,
}

impl Extremum {
    fn name(self) -> &'static str {
        match self {
            Extremum::Min => "MIN",
            Extremum::Max => "MAX",
        }
    }
}

/// The subscript of an `arr` element.
#[derive(Debug, Clone)]
pub enum ElemIndex {
    /// A constant subscript in `0..ARRAY_LEN`.
    Const(i64),
    /// `(e MOD 8 + 8) MOD 8` for a DINT expression `e`; always in range.
    Dynamic(Box<Expr>),
}

/// A typed expression.
#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Literal),
    Var(String),
    Element(ElemIndex),
    /// Unary minus on an integer.
    Neg(Box<Expr>),
    /// `NOT` on a BOOL.
    Not(Box<Expr>),
    Abs(Box<Expr>),
    /// `SQRT` on a real operand; generated only as `SQRT(ABS(x))`.
    Sqrt(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Extremum(Extremum, Box<Expr>, Box<Expr>),
    /// `LIMIT(lo, x, hi)` with constant `lo <= hi`.
    Limit(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `SEL(g, x, y)`.
    Sel(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `FROM_TO_TO(arg)`.
    Convert {
        from: Ty,
        to: Ty,
        arg: Box<Expr>,
    },
    /// `F0(a := x, b := y)`, a call of a generated DINT function.
    Call {
        function: String,
        a: Box<Expr>,
        b: Box<Expr>,
    },
}

/// One label of a `CASE` arm.
#[derive(Debug, Clone)]
pub enum CaseItem {
    Single(i64),
    Range(i64, i64),
}

/// A statement; nested bodies are statement lists.
#[derive(Debug, Clone)]
pub enum Stmt {
    Assign {
        target: String,
        value: Expr,
    },
    AssignElement {
        index: ElemIndex,
        value: Expr,
    },
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },
    /// `CASE selector OF labels: body ... ELSE body END_CASE;`
    Case {
        selector: Expr,
        arms: Vec<(Vec<CaseItem>, Vec<Stmt>)>,
        else_body: Vec<Stmt>,
    },
    /// `FOR i := 0 TO upper DO body END_FOR;`
    For {
        upper: i64,
        body: Vec<Stmt>,
    },
    /// `k := 0; WHILE k < bound DO k := k + 1; body END_WHILE;`
    While {
        bound: i64,
        body: Vec<Stmt>,
    },
}

/// A `VAR` entry of `Main`: `v0 .. vN` with a typed initial literal.
#[derive(Debug, Clone)]
struct StateVar {
    name: String,
    ty: Ty,
    init: Literal,
}

/// A generated `FUNCTION F<n> : DINT` with inputs `a` and `b`.
#[derive(Debug, Clone)]
struct FunctionDecl {
    name: String,
    body: Expr,
}

/// The whole generated program before printing.
#[derive(Debug, Clone)]
struct ProgramAst {
    state: Vec<StateVar>,
    functions: Vec<FunctionDecl>,
    body: Vec<Stmt>,
}

/// The names an expression may read, with their types.
struct Scope {
    variables: Vec<(String, Ty)>,
    /// Whether `arr` elements may be read (`Main` only).
    element_access: bool,
    /// Functions a `Main` expression may call.
    functions: Vec<String>,
}

/// Expression strategies for every type at every depth up to `MAX_EXPR_DEPTH`.
#[derive(Clone)]
struct ExprTable {
    /// `levels[depth][ty.index()]` generates expressions of `ty` of depth at most `depth`.
    levels: Vec<Vec<BoxedStrategy<Expr>>>,
}

impl ExprTable {
    fn new(scope: &Scope) -> Self {
        let mut levels: Vec<Vec<BoxedStrategy<Expr>>> =
            vec![Ty::ALL.iter().map(|&ty| leaf(scope, ty)).collect()];
        for _ in 0..MAX_EXPR_DEPTH {
            let below = levels.last().expect("a level exists");
            let next: Vec<BoxedStrategy<Expr>> =
                Ty::ALL.iter().map(|&ty| node(scope, ty, below)).collect();
            levels.push(next);
        }
        ExprTable { levels }
    }

    fn at(&self, depth: usize, ty: Ty) -> BoxedStrategy<Expr> {
        self.levels[depth][ty.index()].clone()
    }
}

/// A typed literal of `ty` within the generated ranges.
fn literal(ty: Ty) -> BoxedStrategy<Literal> {
    match ty {
        Ty::Bool => any::<bool>().prop_map(Literal::Bool).boxed(),
        Ty::Int | Ty::DInt | Ty::LInt => {
            let (low, high) = ty.integer_bounds();
            (low..=high)
                .prop_map(move |value| Literal::Int { ty, value })
                .boxed()
        }
        Ty::Real | Ty::LReal => (-400..=400_i32)
            .prop_map(move |quarters| Literal::Real { ty, quarters })
            .boxed(),
    }
}

fn integer_literal(ty: Ty, value: i64) -> Expr {
    Expr::Lit(Literal::Int { ty, value })
}

/// Depth-0 expressions of `ty`: a literal, or a variable of that type in scope.
fn leaf(scope: &Scope, ty: Ty) -> BoxedStrategy<Expr> {
    let mut options: Vec<BoxedStrategy<Expr>> = vec![literal(ty).prop_map(Expr::Lit).boxed()];
    for (name, variable_ty) in &scope.variables {
        if *variable_ty == ty {
            options.push(Just(Expr::Var(name.clone())).boxed());
        }
    }
    Union::new(options).boxed()
}

/// Expressions of `ty` one level deeper than `below`, the strategies one level down.
fn node(scope: &Scope, ty: Ty, below: &[BoxedStrategy<Expr>]) -> BoxedStrategy<Expr> {
    let sub = |operand: Ty| below[operand.index()].clone();
    let mut options = vec![leaf(scope, ty)];
    match ty {
        Ty::Bool => {
            options.push(
                sub(Ty::Bool)
                    .prop_map(|operand| Expr::Not(Box::new(operand)))
                    .boxed(),
            );
            for op in [BinOp::And, BinOp::Or, BinOp::Xor] {
                options.push(binary(op, sub(Ty::Bool), sub(Ty::Bool)));
            }
            for operand in Ty::NUMERIC {
                options.push(comparison(sub(operand), sub(operand)));
            }
        }
        Ty::Int | Ty::DInt | Ty::LInt => {
            let own = sub(ty);
            for op in [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::Mod] {
                options.push(binary(op, own.clone(), own.clone()));
            }
            options.push(
                own.clone()
                    .prop_map(|operand| Expr::Neg(Box::new(operand)))
                    .boxed(),
            );
            options.push(
                own.clone()
                    .prop_map(|operand| Expr::Abs(Box::new(operand)))
                    .boxed(),
            );
            for which in [Extremum::Min, Extremum::Max] {
                options.push(extremum(which, own.clone(), own.clone()));
            }
            options.push(limit(ty, own.clone()));
            options.push(selection(sub(Ty::Bool), own.clone(), own.clone()));
            for source in Ty::NUMERIC {
                if source != ty {
                    options.push(convert(source, ty, sub(source)));
                }
            }
            if ty == Ty::DInt {
                if scope.element_access {
                    options.push(element(own.clone()));
                }
                for function in &scope.functions {
                    options.push(call(function.clone(), own.clone(), own.clone()));
                }
            }
        }
        Ty::Real | Ty::LReal => {
            let own = sub(ty);
            for op in [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div] {
                options.push(binary(op, own.clone(), own.clone()));
            }
            options.push(
                own.clone()
                    .prop_map(|operand| Expr::Abs(Box::new(operand)))
                    .boxed(),
            );
            options.push(
                own.clone()
                    .prop_map(|operand| Expr::Sqrt(Box::new(Expr::Abs(Box::new(operand)))))
                    .boxed(),
            );
            for source in Ty::NUMERIC {
                if source != ty {
                    options.push(convert(source, ty, sub(source)));
                }
            }
        }
    }
    Union::new(options).boxed()
}

fn binary(op: BinOp, left: BoxedStrategy<Expr>, right: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (left, right)
        .prop_map(move |(first, second)| Expr::Binary(op, Box::new(first), Box::new(second)))
        .boxed()
}

fn comparison(left: BoxedStrategy<Expr>, right: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    let operator = (0..COMPARISONS.len()).prop_map(|index| COMPARISONS[index]);
    (left, right, operator)
        .prop_map(|(first, second, op)| Expr::Binary(op, Box::new(first), Box::new(second)))
        .boxed()
}

fn extremum(
    which: Extremum,
    left: BoxedStrategy<Expr>,
    right: BoxedStrategy<Expr>,
) -> BoxedStrategy<Expr> {
    (left, right)
        .prop_map(move |(first, second)| Expr::Extremum(which, Box::new(first), Box::new(second)))
        .boxed()
}

/// `LIMIT(lo, value, hi)` with generated constants ordered so that `lo <= hi`.
fn limit(ty: Ty, value: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    let (low, high) = ty.integer_bounds();
    (low..=high, low..=high, value)
        .prop_map(move |(first, second, operand)| {
            let (lower, upper) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            Expr::Limit(
                Box::new(integer_literal(ty, lower)),
                Box::new(operand),
                Box::new(integer_literal(ty, upper)),
            )
        })
        .boxed()
}

fn selection(
    choice: BoxedStrategy<Expr>,
    first: BoxedStrategy<Expr>,
    second: BoxedStrategy<Expr>,
) -> BoxedStrategy<Expr> {
    (choice, first, second)
        .prop_map(|(select, one, other)| {
            Expr::Sel(Box::new(select), Box::new(one), Box::new(other))
        })
        .boxed()
}

fn convert(from: Ty, to: Ty, operand: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    operand
        .prop_map(move |arg| Expr::Convert {
            from,
            to,
            arg: Box::new(arg),
        })
        .boxed()
}

/// `arr[...]`: a constant subscript or the wrapped dynamic one.
fn element_index(dynamic: BoxedStrategy<Expr>) -> BoxedStrategy<ElemIndex> {
    Union::new(vec![
        (0..ARRAY_LEN).prop_map(ElemIndex::Const).boxed(),
        dynamic
            .prop_map(|position| ElemIndex::Dynamic(Box::new(position)))
            .boxed(),
    ])
    .boxed()
}

fn element(dynamic: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    element_index(dynamic).prop_map(Expr::Element).boxed()
}

fn call(function: String, a: BoxedStrategy<Expr>, b: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (a, b)
        .prop_map(move |(first, second)| Expr::Call {
            function: function.clone(),
            a: Box::new(first),
            b: Box::new(second),
        })
        .boxed()
}

/// `Main` sees the state variables, the inputs and `cycles`.
fn main_scope(state: &[StateVar], functions: &[String]) -> Scope {
    let mut variables: Vec<(String, Ty)> = state
        .iter()
        .map(|variable| (variable.name.clone(), variable.ty))
        .collect();
    variables.extend([
        ("in_d0".to_string(), Ty::DInt),
        ("in_d1".to_string(), Ty::DInt),
        ("in_b0".to_string(), Ty::Bool),
        ("in_r0".to_string(), Ty::Real),
        ("cycles".to_string(), Ty::DInt),
    ]);
    Scope {
        variables,
        element_access: true,
        functions: functions.to_vec(),
    }
}

/// A function body sees only its two DINT inputs.
fn function_scope() -> Scope {
    Scope {
        variables: vec![("a".to_string(), Ty::DInt), ("b".to_string(), Ty::DInt)],
        element_access: false,
        functions: Vec::new(),
    }
}

/// What a statement may assign, and the expression strategies it reads with.
struct Generator {
    /// Assignable variables: the state variables.
    targets: Vec<(String, Ty)>,
    table: ExprTable,
}

/// Loops whose counter may not be assigned by a nested statement.
#[derive(Debug, Clone, Copy)]
struct Loops {
    in_for: bool,
    in_while: bool,
}

const NO_LOOPS: Loops = Loops {
    in_for: false,
    in_while: false,
};

impl Loops {
    fn entering_for(self) -> Self {
        Loops {
            in_for: true,
            ..self
        }
    }

    fn entering_while(self) -> Self {
        Loops {
            in_while: true,
            ..self
        }
    }
}

/// A statement at `depth`. Compound statements are generated only above the depth limit.
fn statement(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Stmt> {
    let mut options = vec![assignment(generator), element_assignment(generator)];
    if depth < MAX_STMT_DEPTH {
        let inner = depth + 1;
        options.push(if_statement(generator, inner, loops));
        options.push(case_statement(generator, inner, loops));
        if !loops.in_for {
            options.push(for_statement(generator, inner, loops));
        }
        if !loops.in_while {
            options.push(while_statement(generator, inner, loops));
        }
    }
    Union::new(options).boxed()
}

/// A statement list of one or two statements at `depth`.
fn block(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Vec<Stmt>> {
    prop::collection::vec(statement(generator, depth, loops), 1..=2).boxed()
}

fn assignment(generator: &Generator) -> BoxedStrategy<Stmt> {
    Union::new(generator.targets.iter().map(|(name, ty)| {
        let target = name.clone();
        generator
            .table
            .at(MAX_EXPR_DEPTH, *ty)
            .prop_map(move |value| Stmt::Assign {
                target: target.clone(),
                value,
            })
            .boxed()
    }))
    .boxed()
}

fn element_assignment(generator: &Generator) -> BoxedStrategy<Stmt> {
    let index = element_index(generator.table.at(MAX_EXPR_DEPTH, Ty::DInt));
    let value = generator.table.at(MAX_EXPR_DEPTH, Ty::DInt);
    (index, value)
        .prop_map(|(index, value)| Stmt::AssignElement { index, value })
        .boxed()
}

fn if_statement(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Stmt> {
    let condition = generator.table.at(MAX_EXPR_DEPTH, Ty::Bool);
    let body = block(generator, depth, loops);
    (
        condition.clone(),
        body.clone(),
        condition,
        body.clone(),
        any::<bool>(),
        body.clone(),
        any::<bool>(),
    )
        .prop_map(
            |(first, first_body, second, second_body, with_elsif, else_body, with_else)| {
                let mut branches = vec![(first, first_body)];
                if with_elsif {
                    branches.push((second, second_body));
                }
                Stmt::If {
                    branches,
                    else_body: with_else.then_some(else_body),
                }
            },
        )
        .boxed()
}

/// `CASE` on a DINT expression. The labels are distinct values; consecutive runs become one
/// range or a list, and the arm bodies come from `block`. `ELSE` is always present.
fn case_statement(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Stmt> {
    let selector = generator.table.at(MAX_EXPR_DEPTH, Ty::DInt);
    let arm_body = block(generator, depth, loops);
    let else_body = block(generator, depth, loops);
    (
        selector,
        prop::collection::btree_set(0..=CASE_LABEL_MAX, 1..=5),
        prop::collection::vec(any::<bool>(), 8),
        prop::collection::vec(any::<bool>(), 8),
        else_body,
    )
        .prop_flat_map(move |(selector, labels, breaks, ranges, else_body)| {
            let arms = case_arms(&labels, &breaks, &ranges);
            prop::collection::vec(arm_body.clone(), arms.len()).prop_map(move |bodies| Stmt::Case {
                selector: selector.clone(),
                arms: arms.clone().into_iter().zip(bodies).collect(),
                else_body: else_body.clone(),
            })
        })
        .boxed()
}

/// Splits the sorted labels into arms at the flagged positions. A run of two or more
/// consecutive labels becomes one range when its flag is set, and a list otherwise.
fn case_arms(labels: &BTreeSet<i64>, breaks: &[bool], ranges: &[bool]) -> Vec<Vec<CaseItem>> {
    let mut groups: Vec<Vec<i64>> = Vec::new();
    for (position, &label) in labels.iter().enumerate() {
        if position == 0 || breaks[position] {
            groups.push(Vec::new());
        }
        if let Some(group) = groups.last_mut() {
            group.push(label);
        }
    }
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let consecutive = group.windows(2).all(|pair| pair[1] == pair[0] + 1);
            match (
                consecutive && group.len() >= 2 && ranges[index],
                group.first(),
                group.last(),
            ) {
                (true, Some(&low), Some(&high)) => vec![CaseItem::Range(low, high)],
                _ => group.iter().map(|&label| CaseItem::Single(label)).collect(),
            }
        })
        .collect()
}

/// `FOR i := 0 TO n DO ... END_FOR;` with `n` in `0..=4`.
fn for_statement(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Stmt> {
    let body = block(generator, depth, loops.entering_for());
    (0..=4_i64, body)
        .prop_map(|(upper, body)| Stmt::For { upper, body })
        .boxed()
}

/// `k := 0; WHILE k < n DO k := k + 1; ... END_WHILE;` with `n` in `1..=4`.
fn while_statement(generator: &Generator, depth: u32, loops: Loops) -> BoxedStrategy<Stmt> {
    let body = block(generator, depth, loops.entering_while());
    (1..=4_i64, body)
        .prop_map(|(bound, body)| Stmt::While { bound, body })
        .boxed()
}

/// A state variable's type and typed initial literal.
fn state_variable() -> BoxedStrategy<(Ty, Literal)> {
    (0..Ty::ALL.len())
        .prop_map(|index| Ty::ALL[index])
        .prop_flat_map(|ty| literal(ty).prop_map(move |init| (ty, init)))
        .boxed()
}

/// Assembles the program AST for the given state variables and function count.
fn program_ast(state: Vec<(Ty, Literal)>, function_count: usize) -> BoxedStrategy<ProgramAst> {
    let state: Vec<StateVar> = state
        .into_iter()
        .enumerate()
        .map(|(index, (ty, init))| StateVar {
            name: format!("v{index}"),
            ty,
            init,
        })
        .collect();
    let function_names: Vec<String> = (0..function_count)
        .map(|index| format!("F{index}"))
        .collect();
    let generator = Generator {
        targets: state
            .iter()
            .map(|variable| (variable.name.clone(), variable.ty))
            .collect(),
        table: ExprTable::new(&main_scope(&state, &function_names)),
    };
    let body = prop::collection::vec(statement(&generator, 1, NO_LOOPS), 1..=8);
    let function_table = ExprTable::new(&function_scope());
    let functions =
        prop::collection::vec(function_table.at(MAX_EXPR_DEPTH, Ty::DInt), function_count);
    (functions, body)
        .prop_map(move |(bodies, body)| ProgramAst {
            state: state.clone(),
            functions: function_names
                .iter()
                .zip(bodies)
                .map(|(name, function_body)| FunctionDecl {
                    name: name.clone(),
                    body: function_body,
                })
                .collect(),
            body,
        })
        .boxed()
}

/// One generated program with one input row per cycle.
pub struct ProgramCase {
    pub source: String,
    pub inputs: Vec<[Value; 4]>,
    /// The program behind `source`, kept so that it can be printed again with other options.
    program: ProgramAst,
}

impl ProgramCase {
    /// Prints the program with `options`. `PrintOptions::default()` gives `source`.
    pub fn print(&self, options: PrintOptions) -> String {
        render_with(&self.program, options)
    }
}

impl fmt::Debug for ProgramCase {
    /// Prints the source as text, so a shrunk failure shows the program.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ProgramCase {{\n{}\ninputs: {:?}\n}}",
            self.source, self.inputs
        )
    }
}

/// Input row values: `in_d0`, `in_d1` (DINT), `in_b0` (BOOL), `in_r0` (REAL). The integer
/// extremes, signed zero and a subnormal are included on purpose. NaN and infinity are not: the
/// runtime refuses a non-finite REAL write (`NonFiniteValue`, spec 10 §6.7), so they never reach
/// a program.
fn input_row() -> BoxedStrategy<[Value; 4]> {
    (dint_input(), dint_input(), any::<bool>(), real_input())
        .prop_map(|(first, second, flag, real)| {
            [
                Value::DInt(first),
                Value::DInt(second),
                Value::Bool(flag),
                Value::Real(real),
            ]
        })
        .boxed()
}

fn dint_input() -> BoxedStrategy<i32> {
    prop_oneof![Just(i32::MIN), Just(i32::MAX), Just(0), -64..=64].boxed()
}

fn real_input() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(-0.0_f32),
        Just(1.0e-40_f32),
        Just(f32::MAX),
        Just(-f32::MAX),
        (-400..=400_i32).prop_map(|quarters| quarters as f32 * 0.25),
    ]
    .boxed()
}

/// Generated programs: 3 to 8 state variables, 0 to 2 DINT functions, a body of 1 to 8
/// statements of nesting depth at most 3, and one input row per cycle. Programs over 200 lines
/// are rejected.
pub fn program_case() -> BoxedStrategy<ProgramCase> {
    (
        prop::collection::vec(state_variable(), 3..=8),
        0..=2_usize,
        prop::collection::vec(input_row(), CYCLES),
    )
        .prop_flat_map(|(state, function_count, inputs)| {
            program_ast(state, function_count).prop_map(move |program| ProgramCase {
                source: render(&program),
                inputs: inputs.clone(),
                program,
            })
        })
        .prop_filter("program fits in about 200 lines", |case| {
            case.source.lines().count() <= MAX_LINES
        })
        .boxed()
}
