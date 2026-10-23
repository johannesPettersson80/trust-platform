//! A pair's link check (specification 39, §8.6): each field of its faceplate needs a member of the
//! pair's block, by the pair's map or by the field's own name, that holds a kind of value the field
//! takes. The block is a function block (its inputs, outputs and variables, those of its bases
//! too) or a program (its variables). The check reads only the program's types, so it needs no
//! instance of the block: the compiled program's for the pair file, the language server's symbol
//! table for the block's ST file, so both files show the same problems (§8.6). A member beside the
//! instance (`{instance}_Setpoint`, §8.5) is no member of the block: it is checked where an
//! instance is bound, against the variables the program declares.

use trust_hir::{Type, TypeId};

use crate::hmi::catalogue::ValueKind;
use crate::hmi::page_files::{beside_instance, HmiFaceplate, HmiPair};
use crate::program_model::FunctionBlockBase;
use crate::runtime::RuntimeMetadata;

/// A problem of a pair: of one of its faceplate's fields, or of its block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairProblem {
    /// The field, or `None` for the pair's `block`.
    pub field: Option<String>,
    /// The symbol's property, for a problem of a `[bind]` entry.
    pub binding: Option<String>,
    /// What is wrong.
    pub message: String,
}

/// What the pair check reads of a program: the members of its blocks and the types they hold.
pub trait PairProgram {
    /// The members of the function block or program a pair's `block` names (by
    /// [`block_named`]), a function block's bases' too, by name with their types; `None` when the
    /// program has neither.
    fn block_members(&self, name: &str) -> Option<Vec<(String, TypeId)>>;
    /// A type by its id.
    fn type_of(&self, id: TypeId) -> Option<&Type>;
}

/// The name among `names` that a pair's `block` names: a qualified name names its own block; a
/// name without a namespace may name a block in one.
pub fn block_named<'a>(names: impl IntoIterator<Item = &'a str>, name: &str) -> Option<&'a str> {
    let short = |full: &str| full.rsplit('.').next().unwrap_or(full).to_ascii_lowercase();
    let names: Vec<&str> = names.into_iter().collect();
    let exact = names
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name));
    let unqualified = || {
        (!name.contains('.'))
            .then(|| {
                names
                    .iter()
                    .find(|candidate| short(candidate) == short(name))
            })
            .flatten()
    };
    exact.or_else(unqualified).copied()
}

/// Checks a pair and its faceplate against a program.
pub fn check_pair(
    pair: &HmiPair,
    faceplate: &HmiFaceplate,
    program: &(impl PairProgram + ?Sized),
) -> Vec<PairProblem> {
    let Some(members) = program.block_members(&pair.block) else {
        return vec![PairProblem {
            field: None,
            binding: None,
            message: format!(
                "the program has no function block or program `{}`",
                pair.block
            ),
        }];
    };
    let block = &pair.block;
    let mut problems: Vec<PairProblem> = faceplate
        .fields
        .iter()
        .filter_map(|(field, definition)| {
            let member = pair.fields.get(field).map(String::as_str).unwrap_or(field);
            if beside_instance(member) {
                return None;
            }
            let problem = |message: String| Some(PairProblem { field: Some(field.clone()), binding: None, message });
            let Some(type_id) = member_type(program, &members, member) else {
                return problem(format!("`{block}` has no member `{member}`, which the faceplate's field `{field}` needs"));
            };
            let accepted: Vec<ValueKind> = definition.accepts.iter().filter_map(|kind| kind_named(kind)).collect();
            if accepted.is_empty() {
                return None;
            }
            let held = kind_of(program, type_id);
            if held.is_some_and(|held| accepted.iter().any(|kind| takes(*kind, held))) {
                return None;
            }
            let holds = held.map_or("a value no field takes", ValueKind::words);
            let takes = accepted.iter().map(|kind| kind.words()).collect::<Vec<_>>().join(" or ");
            problem(format!("the member `{member}` of `{block}` holds {holds}; the field `{field}` takes {takes}"))
        })
        .collect();
    // Each `[bind]` entry: a property of the pair's symbol, shown by a member of a kind it takes.
    let component = crate::hmi::catalogue::Catalogue::shipped().component(&pair.symbol);
    for (property, member) in &pair.bind {
        let problem = |message: String| PairProblem {
            field: None,
            binding: Some(property.clone()),
            message,
        };
        let Some(definition) = component.and_then(|component| component.properties.get(property))
        else {
            problems.push(problem(format!(
                "the symbol `{}` has no property `{property}`",
                pair.symbol
            )));
            continue;
        };
        if beside_instance(member) {
            continue;
        }
        let Some(type_id) = member_type(program, &members, member) else {
            problems.push(problem(format!(
                "`{block}` has no member `{member}`, which the symbol's `{property}` shows"
            )));
            continue;
        };
        let held = kind_of(program, type_id);
        if held.is_some_and(|held| definition.accepts.iter().any(|kind| takes(*kind, held))) {
            continue;
        }
        let holds = held.map_or("a value no property takes", ValueKind::words);
        let takes = definition
            .accepts
            .iter()
            .map(|kind| kind.words())
            .collect::<Vec<_>>()
            .join(" or ");
        problems.push(problem(format!("the member `{member}` of `{block}` holds {holds}; the symbol's `{property}` takes {takes}")));
    }
    problems.extend(check_definition_members(pair, program, &members));
    problems
}

/// Each setting, hardware terminal, dependency and initial value of a definition names a member of
/// its block that can hold it (§8.5): a setting a single value, a terminal a BOOL or a number, a
/// terminal's `required_when` a BOOL.
fn check_definition_members(
    pair: &HmiPair,
    program: &(impl PairProgram + ?Sized),
    members: &[(String, TypeId)],
) -> Vec<PairProblem> {
    let block = &pair.block;
    let kind = |member: &str| member_type(program, members, member).map(|ty| kind_of(program, ty));
    // A member beside the instance is checked where an instance is bound (§8.5).
    let own = |member: &str| !beside_instance(member);
    let problem = |message: String| PairProblem {
        field: None,
        binding: None,
        message,
    };
    let mut problems = Vec::new();
    for setting in pair.settings.iter().filter(|setting| own(&setting.member)) {
        match kind(&setting.member) {
            None => problems.push(problem(format!(
                "`{block}` has no member `{}`, which a setting names",
                setting.member
            ))),
            Some(Some(
                ValueKind::Boolean
                | ValueKind::Number
                | ValueKind::Time
                | ValueKind::Text
                | ValueKind::Enumeration,
            )) => {}
            Some(_) => problems.push(problem(format!(
                "the setting `{}` of `{block}` does not hold a single value",
                setting.member
            ))),
        }
        if let Some(unit) = setting
            .unit
            .as_deref()
            .and_then(|unit| unit.strip_prefix('$'))
            && kind(unit) != Some(Some(ValueKind::Text))
        {
            problems.push(problem(format!(
                    "the unit of the setting `{}` names `{unit}`, which is not a text member of `{block}`",
                    setting.member
                )));
        }
    }
    for terminal in &pair.io {
        match kind(&terminal.member) {
            None => problems.push(problem(format!(
                "`{block}` has no member `{}`, which a hardware terminal names",
                terminal.member
            ))),
            Some(Some(ValueKind::Boolean | ValueKind::Number)) => {}
            Some(_) => problems.push(problem(format!(
                "the hardware terminal `{}` of `{block}` is not a BOOL or a number",
                terminal.member
            ))),
        }
        if let Some(condition) = &terminal.required_when
            && kind(condition) != Some(Some(ValueKind::Boolean))
        {
            problems.push(problem(format!(
                    "`required_when` of `{}` names `{condition}`, which is not a BOOL member of `{block}`",
                    terminal.member
                )));
        }
    }
    for dependency in pair
        .dependencies
        .iter()
        .filter(|dependency| own(&dependency.member))
    {
        if kind(&dependency.member).is_none() {
            problems.push(problem(format!(
                "`{block}` has no member `{}`, which a dependency names",
                dependency.member
            )));
        }
    }
    for member in pair.program.iter().flat_map(|rules| rules.initial.keys()) {
        if own(member) && kind(member).is_none() {
            problems.push(problem(format!(
                "`{block}` has no member `{member}`, which an initial value names"
            )));
        }
    }
    problems
}

/// The compiled program: a block's members by name with their types, a function block's with its
/// bases', or a program's.
impl PairProgram for RuntimeMetadata {
    fn block_members(&self, name: &str) -> Option<Vec<(String, TypeId)>> {
        if let Some(found) =
            block_named(self.function_blocks().keys().map(|key| key.as_str()), name)
        {
            let mut members = Vec::new();
            let mut next = Some(found.to_string());
            let mut seen = Vec::new();
            // A derived block has its bases' members too (IEC 61131-3 Ed.3 §6.6.5.3); a base is
            // named as declared, which the compiled names need not match in case or namespace.
            while let Some(current) = next.take().filter(|current| !seen.contains(current)) {
                let Some(block) = self.function_blocks().get(current.as_str()) else {
                    break;
                };
                members.extend(
                    block
                        .params
                        .iter()
                        .map(|param| (param.name.to_string(), param.type_id)),
                );
                members.extend(
                    block
                        .vars
                        .iter()
                        .map(|var| (var.name.to_string(), var.type_id)),
                );
                next = match &block.base {
                    Some(FunctionBlockBase::FunctionBlock(base)) => {
                        block_named(self.function_blocks().keys().map(|key| key.as_str()), base)
                            .map(str::to_string)
                    }
                    _ => None,
                };
                seen.push(current);
            }
            return Some(members);
        }
        let found = block_named(
            self.program_templates().keys().map(|key| key.as_str()),
            name,
        )?;
        let program = self.program_templates().get(found)?;
        Some(
            program
                .vars
                .iter()
                .map(|var| (var.name.to_string(), var.type_id))
                .collect(),
        )
    }

    fn type_of(&self, id: TypeId) -> Option<&Type> {
        self.registry().get(id)
    }
}

/// The type of a member path, `Member` or `Member.Field…` through structures and the instances
/// a block holds (`FailedToStart.Active`).
fn member_type(
    program: &(impl PairProgram + ?Sized),
    members: &[(String, TypeId)],
    path: &str,
) -> Option<TypeId> {
    let mut segments = path.split('.');
    let (first, indices) = indexed(segments.next()?)?;
    let found = members
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(first))?
        .1;
    let mut type_id = elements(program, found, &indices)?;
    for segment in segments {
        let (segment, indices) = indexed(segment)?;
        let member = match named(program, type_id)? {
            Type::Struct { fields, .. } => {
                fields
                    .iter()
                    .find(|field| field.name.eq_ignore_ascii_case(segment))?
                    .type_id
            }
            Type::FunctionBlock { name } => {
                program
                    .block_members(name)?
                    .into_iter()
                    .find(|(member, _)| member.eq_ignore_ascii_case(segment))?
                    .1
            }
            _ => return None,
        };
        type_id = elements(program, member, &indices)?;
    }
    Some(type_id)
}

/// A path segment's name and its index groups (`Selected[1]`, `Grid[1, 2]`, `Rows[1][2]`);
/// `None` when a bracket is unbalanced or an index is not a whole number.
fn indexed(segment: &str) -> Option<(&str, Vec<Vec<i64>>)> {
    let Some(open) = segment.find('[') else {
        return Some((segment, Vec::new()));
    };
    let (name, mut rest) = segment.split_at(open);
    let mut groups = Vec::new();
    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let close = inner.find(']')?;
        let indices = inner[..close]
            .split(',')
            .map(|index| index.trim().parse::<i64>().ok())
            .collect::<Option<Vec<_>>>()?;
        groups.push(indices);
        rest = &inner[close + 1..];
    }
    Some((name, groups))
}

/// The type that index groups select in an array, one array level per group; `None` when a
/// group does not match its array's dimensions or an index lies outside its bounds.
fn elements(
    program: &(impl PairProgram + ?Sized),
    mut type_id: TypeId,
    groups: &[Vec<i64>],
) -> Option<TypeId> {
    for indices in groups {
        match named(program, type_id)? {
            Type::Array {
                element,
                dimensions,
            } if dimensions.len() == indices.len()
                && dimensions
                    .iter()
                    .zip(indices)
                    .all(|((low, high), index)| (low..=high).contains(&index)) =>
            {
                type_id = *element;
            }
            _ => return None,
        }
    }
    Some(type_id)
}

/// A type, through the names given to it (`TYPE Rpm : Speed; END_TYPE`).
fn named(program: &(impl PairProgram + ?Sized), type_id: TypeId) -> Option<&Type> {
    let mut current = program.type_of(type_id)?;
    // A name for a name for a type; a chain longer than the registry is a loop.
    for _ in 0..64 {
        match current {
            Type::Alias { target, .. } => current = program.type_of(*target)?,
            other => return Some(other),
        }
    }
    None
}

/// The kind of value a type holds, as the catalogue names it (§5.6).
fn kind_of(program: &(impl PairProgram + ?Sized), type_id: TypeId) -> Option<ValueKind> {
    Some(match named(program, type_id)? {
        Type::Bool => ValueKind::Boolean,
        Type::SInt
        | Type::Int
        | Type::DInt
        | Type::LInt
        | Type::USInt
        | Type::UInt
        | Type::UDInt
        | Type::ULInt => ValueKind::Number,
        Type::Real | Type::LReal | Type::Byte | Type::Word | Type::DWord | Type::LWord => {
            ValueKind::Number
        }
        Type::Time
        | Type::LTime
        | Type::Date
        | Type::LDate
        | Type::Tod
        | Type::LTod
        | Type::Dt
        | Type::Ldt => ValueKind::Time,
        Type::String { .. } | Type::WString { .. } | Type::Char | Type::WChar => ValueKind::Text,
        Type::Enum { .. } => ValueKind::Enumeration,
        Type::Struct { .. } | Type::FunctionBlock { .. } | Type::Class { .. } => {
            ValueKind::Structure
        }
        Type::Array { .. } => ValueKind::Array,
        Type::Subrange { base, .. } => return kind_of(program, *base),
        _ => return None,
    })
}

/// Whether a field taking `kind` takes a member holding `held`.
fn takes(kind: ValueKind, held: ValueKind) -> bool {
    kind == held
        || (kind == ValueKind::Value && !matches!(held, ValueKind::Structure | ValueKind::Array))
}

fn kind_named(name: &str) -> Option<ValueKind> {
    serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
}
