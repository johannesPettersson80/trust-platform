//! The admission of a program's Rust work at build (specification 48 RU-46, RU-47). For each task,
//! the build adds up the budgets of the Rust POUs the task runs, each times the most calls one scan
//! can make of it: one outside a loop, the count of a `FOR` loop with constant bounds, and no bound
//! inside another loop. The calls are followed through the Structured Text that makes them: its
//! programs, function blocks and functions, and their methods, actions and properties. A call that
//! can run one of several bodies (a method that a derived type overrides, a method of an interface,
//! the body of a function block that another extends) counts the most Rust work among them, as the
//! build knows every type of the program. A task whose Rust work so counted exceeds its interval,
//! or that calls a Rust POU in a loop without a bound, gets a finding, which the deployment's
//! profile makes a warning or an error.

use std::collections::HashMap;

use trust_hir::symbols::{Symbol, SymbolKind, SymbolTable, VarQualifier};
use trust_syntax::syntax::{SyntaxKind as K, SyntaxNode};

use super::classes::Owner;
use super::front::Front;
use super::lower::{is_expression, qualified};
use super::model::{BlockTask, Model, Task};
use super::types::Kind;
use crate::program_model::MethodDef;
use crate::task::ProgramDef;

/// How many times one scan can reach a place: a count, or no bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Times {
    Count(u128),
    Unbounded,
}

impl Times {
    /// `self` times, each `other` times; never is never, even in a loop without a bound.
    fn times(self, other: Times) -> Times {
        match (self, other) {
            (Times::Count(0), _) | (_, Times::Count(0)) => Times::Count(0),
            (Times::Count(left), Times::Count(right)) => left
                .checked_mul(right)
                .map_or(Times::Unbounded, Times::Count),
            _ => Times::Unbounded,
        }
    }

    /// `self` times and `other` times more.
    fn plus(self, other: Times) -> Times {
        match (self, other) {
            (Times::Count(left), Times::Count(right)) => left
                .checked_add(right)
                .map_or(Times::Unbounded, Times::Count),
            _ => Times::Unbounded,
        }
    }

    /// The more of the two.
    fn most(self, other: Times) -> Times {
        match (self, other) {
            (Times::Count(left), Times::Count(right)) => Times::Count(left.max(right)),
            _ => Times::Unbounded,
        }
    }
}

/// The Rust work of one run of a body: the calls of each Rust POU, by its name, and how many times
/// one run makes each.
#[derive(Clone, Debug, Default)]
struct Work {
    calls: Vec<(String, Times)>,
}

impl Work {
    /// The work of one call of the Rust POU `name`.
    fn call_of(name: &str) -> Work {
        Work {
            calls: vec![(name.to_string(), Times::Count(1))],
        }
    }

    /// Adds `times` calls of the Rust POU `name`.
    fn add(&mut self, name: &str, times: Times) {
        match self
            .calls
            .iter_mut()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
        {
            Some((_, total)) => *total = total.plus(times),
            None => self.calls.push((name.to_string(), times)),
        }
    }

    /// Adds the work of `runs` runs of a body whose one run does `work`.
    fn add_runs(&mut self, work: &Work, runs: Times) {
        for (name, times) in &work.calls {
            self.add(name, runs.times(*times));
        }
    }

    /// Takes in `other`, the work of a body that a call may run instead: for each Rust POU, the
    /// more calls of the two.
    fn or(&mut self, other: &Work) {
        for (name, times) in &other.calls {
            match self
                .calls
                .iter_mut()
                .find(|(known, _)| known.eq_ignore_ascii_case(name))
            {
                Some((_, mine)) => *mine = mine.most(*times),
                None => self.calls.push((name.clone(), *times)),
            }
        }
    }
}

/// The budgets of the project's Rust POUs, by their names in capitals, in nanoseconds.
pub(super) type Budgets = HashMap<String, u64>;

/// The instance that a call of a method or of a function block's body runs on, as far as the
/// call's code shows it.
#[derive(Clone, Copy)]
enum Receiver<'a> {
    /// An instance of exactly this type: a variable of it, or the caller's base (`SUPER^`).
    Exact(Owner<'a>),
    /// An instance of this type or of a type derived from it: one that a reference, a
    /// `VAR_IN_OUT`, an input, an array element or `THIS` names (IEC §6.6.8).
    Derived(Owner<'a>),
    /// An instance of any type that implements this interface (IEC §6.6.6).
    Interface(&'a str),
    /// A program instance, whose methods and actions have one form each.
    Program(&'a ProgramDef),
}

struct Admission<'f, 'a> {
    front: &'f Front,
    model: &'f Model<'a>,
    budgets: &'f Budgets,
    /// The work of each body already walked, by its kind or owner and its name in capitals.
    walked: HashMap<String, Work>,
    /// The bodies being walked, so that a recursion, which the build refuses, ends.
    walking: Vec<String>,
}

/// The findings of RU-47 for the tasks of `model`, the Rust POUs' budgets `budgets` applied.
pub(super) fn findings(front: &Front, model: &Model<'_>, budgets: &Budgets) -> Vec<String> {
    let mut admission = Admission {
        front,
        model,
        budgets,
        walked: HashMap::new(),
        walking: Vec::new(),
    };
    let mut findings = Vec::new();
    for task in &model.tasks {
        let work = admission.task(task);
        let task_name = if task.name.is_empty() {
            "the programs without a task".to_string()
        } else {
            format!("the task {}", task.name)
        };
        let mut total: u128 = 0;
        let mut breakdown = Vec::new();
        for (name, times) in &work.calls {
            let budget = u128::from(
                budgets
                    .get(&name.to_ascii_uppercase())
                    .copied()
                    .unwrap_or(0),
            );
            match times {
                Times::Count(0) => {}
                Times::Count(count) => {
                    total = total.saturating_add(budget.saturating_mul(*count));
                    breakdown.push(format!("{name} {count} × {}", duration(budget)));
                }
                Times::Unbounded => findings.push(format!(
                    "{task_name} calls the Rust POU {name} in a loop without a bound, so its Rust \
                     work per scan has no bound (specification 48 RU-47)"
                )),
            }
        }
        if task.interval_ns > 0 && total > u128::from(task.interval_ns) {
            findings.push(format!(
                "{task_name} may spend {} per scan in Rust POUs ({}), more than its interval {} \
                 (specification 48 RU-47)",
                duration(total),
                breakdown.join(", "),
                duration(u128::from(task.interval_ns))
            ));
        }
    }
    findings
}

/// The Rust POUs and methods that the scans of the tasks of `model` can call, by their names in
/// capitals, each once: those whose budgets RU-47's sum counts, of which a `production` build needs
/// a current measurement (specification 48 RU-48).
pub(super) fn reached(front: &Front, model: &Model<'_>, budgets: &Budgets) -> Vec<String> {
    let mut admission = Admission {
        front,
        model,
        budgets,
        walked: HashMap::new(),
        walking: Vec::new(),
    };
    let mut reached: Vec<String> = model
        .tasks
        .iter()
        .flat_map(|task| admission.task(task).calls)
        .filter(|(_, times)| *times != Times::Count(0))
        .map(|(name, _)| name.to_ascii_uppercase())
        .collect();
    reached.sort();
    reached.dedup();
    reached
}

impl<'a> Admission<'_, 'a> {
    /// The Rust work of one scan of `task`: its programs, then the function block instances it runs
    /// on their own (IEC §6.8.2), each once a scan.
    fn task(&mut self, task: &Task) -> Work {
        let model = self.model;
        let mut work = Work::default();
        for &instance in &task.programs {
            let Some(program) = model
                .instances
                .get(instance)
                .and_then(|instance| model.program_types.get(instance.program))
            else {
                continue;
            };
            let body = self.pou(K::Program, &program.def.name);
            work.add_runs(&body, Times::Count(1));
        }
        for &block in &task.blocks {
            let Some(def) = model
                .block_tasks
                .get(block)
                .and_then(|block| block_type(model, block))
            else {
                continue;
            };
            let body = self.pou(K::FunctionBlock, &def);
            work.add_runs(&body, Times::Count(1));
        }
        work
    }

    /// The work of one run of the program, function block or function named `name` (its name in
    /// the program model) of kind `kind`: a call of it for a Rust POU, else its statements' calls.
    fn pou(&mut self, kind: K, name: &str) -> Work {
        if self.budgets.contains_key(&name.to_ascii_uppercase()) {
            return Work::call_of(name);
        }
        let key = format!("{kind:?} {}", name.to_ascii_uppercase());
        // Its statements may stand before and after its methods and actions, each run of them a
        // statement list of its own (specification 04 §8); the members run only when called.
        self.walked_once(key, |front| {
            let (file, node) = super::emit::find_pou(front, kind, name, None)?;
            let lists = node
                .children()
                .filter(|child| child.kind() == K::StmtList)
                .collect();
            Some((file, lists))
        })
    }

    /// The work of one run of `method` of the type or program named `owner`: a method, a textual
    /// action, or a property's `GET` or `SET`.
    fn member(&mut self, owner: &str, method: &MethodDef) -> Work {
        let key = format!(
            "{}.{}",
            owner.to_ascii_uppercase(),
            method.name.to_ascii_uppercase()
        );
        // A Rust function block's method is a call of its budget (specification 48 RU-32).
        if self.budgets.contains_key(&key) {
            return Work::call_of(&key);
        }
        self.walked_once(key, |front| {
            let (file, list) = super::emit::method_body(front, owner, method)?;
            Some((file, list.into_iter().collect()))
        })
    }

    /// The work of one run of the statement lists that `locate` finds, walked once and kept under
    /// `key`.
    fn walked_once(
        &mut self,
        key: String,
        locate: impl FnOnce(&Front) -> Option<(usize, Vec<SyntaxNode>)>,
    ) -> Work {
        if let Some(work) = self.walked.get(&key) {
            return work.clone();
        }
        if self.walking.contains(&key) {
            return Work::default();
        }
        let Some((file, lists)) = locate(self.front) else {
            return Work::default();
        };
        self.walking.push(key.clone());
        let mut work = Work::default();
        for list in &lists {
            self.walk(file, list, Times::Count(1), &mut work);
        }
        self.walking.pop();
        self.walked.insert(key, work.clone());
        work
    }

    /// Adds to `work` the calls in `node`, which one run of its body reaches `times` times.
    fn walk(&mut self, file: usize, node: &SyntaxNode, times: Times, work: &mut Work) {
        match node.kind() {
            // The bounds and the step are evaluated once as the loop starts (specification 10
            // §5.3); its statements, which stand in it directly, once a pass.
            K::ForStmt => {
                let passes = times.times(for_count(node).map_or(Times::Unbounded, Times::Count));
                for child in node.children() {
                    let reached = if child.kind() == K::Name || is_expression(child.kind()) {
                        times
                    } else {
                        passes
                    };
                    self.walk(file, &child, reached, work);
                }
            }
            K::WhileStmt | K::RepeatStmt => {
                for child in node.children() {
                    self.walk(file, &child, times.times(Times::Unbounded), work);
                }
            }
            kind => {
                match kind {
                    K::CallExpr => self.call(file, node, times, work),
                    K::NameRef | K::FieldExpr => self.property(file, node, times, work),
                    _ => {}
                }
                for child in node.children() {
                    self.walk(file, &child, times, work);
                }
            }
        }
    }

    /// A call: the body it runs counts its calls `times` each, a Rust POU's call itself.
    fn call(&mut self, file: usize, node: &SyntaxNode, times: Times, work: &mut Work) {
        let Some(callee) = node.children().next() else {
            return;
        };
        let front = self.front;
        let model = self.model;
        let source = &front.sources[file];
        let symbols = &source.analysis.symbols;
        let symbol = source.symbol_of(&callee).and_then(|id| symbols.get(id));
        let body = match symbol {
            Some(symbol) if matches!(symbol.kind, SymbolKind::Function { .. }) => {
                // A standard function has no definition in the program model and runs no POU.
                let Some(def) = model.function(&qualified(symbols, symbol)) else {
                    return;
                };
                self.pou(K::Function, &def.name)
            }
            Some(symbol)
                if matches!(symbol.kind, SymbolKind::Method { .. } | SymbolKind::Action) =>
            {
                let receiver = match callee.kind() {
                    K::FieldExpr => callee
                        .children()
                        .next()
                        .and_then(|base| self.receiver(file, &base)),
                    _ => self.own_receiver(symbols, symbol),
                };
                let Some(receiver) = receiver else {
                    return;
                };
                self.member_call(receiver, &symbol.name)
            }
            // A function block's body, on the instance that the callee names.
            _ => {
                let Some(receiver) = self.receiver(file, &callee) else {
                    return;
                };
                self.block_call(receiver)
            }
        };
        work.add_runs(&body, times);
    }

    /// A read or a write of a property: the more calls of its `GET` and its `SET` count `times`.
    fn property(&mut self, file: usize, node: &SyntaxNode, times: Times, work: &mut Work) {
        // The member of `instance.Property` counts as the whole expression.
        if node.kind() == K::NameRef
            && node.parent().is_some_and(|parent| {
                parent.kind() == K::FieldExpr && parent.children().next().as_ref() != Some(node)
            })
        {
            return;
        }
        let front = self.front;
        let source = &front.sources[file];
        let symbols = &source.analysis.symbols;
        let Some(symbol) = source.symbol_of(node).and_then(|id| symbols.get(id)) else {
            return;
        };
        if !matches!(symbol.kind, SymbolKind::Property { .. }) {
            return;
        }
        let receiver = match node.kind() {
            K::FieldExpr => node
                .children()
                .next()
                .and_then(|base| self.receiver(file, &base)),
            _ => self.own_receiver(symbols, symbol),
        };
        let Some(receiver) = receiver else {
            return;
        };
        // The declarations hold a property's `GET` as a method named like it and its `SET` as one
        // named `__set_<property>`.
        let mut access = self.member_call(receiver, &symbol.name);
        let set = self.member_call(receiver, &format!("__set_{}", symbol.name));
        access.or(&set);
        work.add_runs(&access, times);
    }

    /// The work of a call of the member `name` (a method, an action or a property's accessor) on
    /// the instance `receiver`: the most among the bodies the call can run, each type's own or
    /// inherited one.
    fn member_call(&mut self, receiver: Receiver<'a>, name: &str) -> Work {
        let defs = self.model.types.defs;
        // The member a call on an instance of `owner` reaches, with the type that declares it.
        let reached = |owner: Owner<'a>| {
            defs.method(owner, name)
                .map(|(declaring, method)| (declaring.name().as_str(), method))
        };
        let bodies: Vec<(&'a str, &'a MethodDef)> = match receiver {
            Receiver::Program(def) => def
                .methods
                .iter()
                .filter(|method| method.name.eq_ignore_ascii_case(name))
                .map(|method| (def.name.as_str(), method))
                .collect(),
            Receiver::Exact(owner) => reached(owner).into_iter().collect(),
            Receiver::Derived(owner) => defs
                .owners()
                .filter(|other| defs.is_a(*other, owner))
                .filter_map(reached)
                .collect(),
            Receiver::Interface(interface) => defs
                .owners()
                .filter(|other| defs.implements(*other, interface))
                .filter_map(reached)
                .collect(),
        };
        let mut work = Work::default();
        for (owner, method) in bodies {
            let body = self.member(owner, method);
            work.or(&body);
        }
        work
    }

    /// The work of a call of a function block's body on the instance `receiver`: the most among
    /// the bodies it can run, its type's own and those of the blocks derived from it (IEC
    /// §6.6.8.3).
    fn block_call(&mut self, receiver: Receiver<'a>) -> Work {
        let defs = self.model.types.defs;
        let blocks: Vec<&'a str> = match receiver {
            Receiver::Exact(owner) => vec![owner.name().as_str()],
            Receiver::Derived(owner) => defs
                .owners()
                .filter(|other| defs.is_a(*other, owner))
                .map(|other| other.name().as_str())
                .collect(),
            Receiver::Interface(_) | Receiver::Program(_) => Vec::new(),
        };
        let mut work = Work::default();
        for block in blocks {
            let body = self.pou(K::FunctionBlock, block);
            work.or(&body);
        }
        work
    }

    /// The instance that the expression `node` names, as a call on it can reach its type's bodies.
    fn receiver(&self, file: usize, node: &SyntaxNode) -> Option<Receiver<'a>> {
        let model = self.model;
        let source = &self.front.sources[file];
        let symbols = &source.analysis.symbols;
        let symbol = source.symbol_of(node).and_then(|id| symbols.get(id));
        let ty = source
            .type_of(node)
            .or_else(|| symbol.map(|symbol| symbol.type_id))?;
        let name = symbols.type_name(ty)?;
        let defs = model.types.defs;
        if let Some(owner) = defs.owner(&name) {
            // `SUPER^` is the caller's base itself (IEC §6.6.5.5); a variable of the type holds an
            // instance of it, unless it is an input or an in-out, which the caller fills.
            let exact = is_super(node)
                || matches!(node.kind(), K::NameRef | K::FieldExpr)
                    && symbol.is_some_and(|symbol| holds_instance(symbols, symbol, &name));
            return Some(if exact {
                Receiver::Exact(owner)
            } else {
                Receiver::Derived(owner)
            });
        }
        if let Some(interface) = defs.interface(&name) {
            return Some(Receiver::Interface(interface.name.as_str()));
        }
        // `Program.m()`, which the CODESYS profile reaches (docs/IEC_DEVIATIONS.md DEV-030).
        model
            .program_types
            .iter()
            .find(|program| program.def.name.eq_ignore_ascii_case(&name))
            .map(|program| Receiver::Program(program.def))
    }

    /// The instance that the member `symbol`, named without one, runs on: its program's, or that
    /// of the block or the class that calls it, which may be of a derived type (IEC §6.6.5.4).
    fn own_receiver(&self, symbols: &SymbolTable, symbol: &Symbol) -> Option<Receiver<'a>> {
        let owner = symbols.get(symbol.parent?)?;
        let name = qualified(symbols, owner);
        let model = self.model;
        if matches!(owner.kind, SymbolKind::Program) {
            return model
                .program_types
                .iter()
                .find(|program| program.def.name.eq_ignore_ascii_case(&name))
                .map(|program| Receiver::Program(program.def));
        }
        model.types.defs.owner(&name).map(Receiver::Derived)
    }
}

/// Whether `node` is `SUPER^` or `SUPER`.
fn is_super(node: &SyntaxNode) -> bool {
    match node.kind() {
        K::SuperExpr => true,
        K::DerefExpr => node
            .children()
            .next()
            .is_some_and(|inner| inner.kind() == K::SuperExpr),
        _ => false,
    }
}

/// Whether the variable `symbol` holds an instance of the type named `ty` itself: one it declares
/// of that type, not an input or an in-out that a caller fills, which may be of a derived type.
fn holds_instance(symbols: &SymbolTable, symbol: &Symbol, ty: &str) -> bool {
    matches!(
        &symbol.kind,
        SymbolKind::Variable { qualifier }
            if !matches!(
                qualifier,
                VarQualifier::Input | VarQualifier::InOut | VarQualifier::Access
            )
    ) && symbols
        .type_name(symbol.type_id)
        .is_some_and(|declared| declared.eq_ignore_ascii_case(ty))
}

/// The function block that a task runs on its own (IEC §6.8.2), by its name in the program model;
/// `None` for a standard one, which runs no POU.
fn block_type(model: &Model<'_>, block: &BlockTask) -> Option<smol_str::SmolStr> {
    let (_, ty) = super::entries::block_instance(model, block)?;
    match &model.types.info(ty).kind {
        Kind::Fb { def, .. } => Some(def.clone()),
        _ => None,
    }
}

/// The number of passes of the `FOR` loop `node` when its bounds and its step are integer literals:
/// `(end − start) / step + 1`, 0 when the loop does not run; `None` for another loop.
fn for_count(node: &SyntaxNode) -> Option<u128> {
    let mut expressions = node.children().filter(|child| is_expression(child.kind()));
    let start = integer(&expressions.next()?)?;
    let end = integer(&expressions.next()?)?;
    let step = match expressions.next() {
        Some(step) => integer(&step)?,
        None => 1,
    };
    if step == 0 {
        return None;
    }
    let passes = end.checked_sub(start)?.checked_div(step)?.checked_add(1)?;
    Some(u128::try_from(passes.max(0)).unwrap_or(0))
}

/// The value of an integer literal, `-` and a typed or based form included (`INT#5`, `16#FF`,
/// `INT#16#FF`).
fn integer(node: &SyntaxNode) -> Option<i128> {
    match node.kind() {
        K::Literal => {
            let text: String = node.text().to_string().replace('_', "");
            let text = text.trim();
            let (base, digits) = match text.rsplit_once('#') {
                Some((prefix, digits)) => (prefix.rsplit('#').next().unwrap_or(prefix), digits),
                None => ("10", text),
            };
            // A type's name before the digits (`INT#5`) leaves them decimal.
            let radix = match base {
                "16" => 16,
                "8" => 8,
                "2" => 2,
                _ => 10,
            };
            i128::from_str_radix(digits, radix).ok()
        }
        K::UnaryExpr => {
            let negated = node
                .children_with_tokens()
                .any(|element| element.kind() == K::Minus);
            let value = integer(&node.children().next()?)?;
            Some(if negated { -value } else { value })
        }
        K::ParenExpr => integer(&node.children().next()?),
        _ => None,
    }
}

/// A duration of `nanos` nanoseconds in the unit that reads best.
fn duration(nanos: u128) -> String {
    if nanos >= 1_000_000 && nanos.is_multiple_of(1_000) {
        format!("{} ms", format_scaled(nanos, 1_000_000))
    } else if nanos >= 1_000 && nanos.is_multiple_of(10) {
        format!("{} µs", format_scaled(nanos, 1_000))
    } else {
        format!("{nanos} ns")
    }
}

fn format_scaled(nanos: u128, scale: u128) -> String {
    let whole = nanos / scale;
    let rest = nanos % scale;
    if rest == 0 {
        return whole.to_string();
    }
    let digits = scale.to_string().len() - 1;
    let fraction = format!("{rest:0digits$}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}

#[cfg(test)]
#[path = "rust_budget_tests.rs"]
mod tests;
