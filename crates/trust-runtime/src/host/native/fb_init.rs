//! CODESYS's implicit `FB_init` (specification 10 §4.8; specification 44 §6.7): under the CODESYS
//! profiles the cold-start and the warm-start initializations call `FB_init` of every instance
//! whose type declares it, after the `REF` initializers, and then each of its methods marked
//! `{attribute 'call_after_init'}` when its block is marked too. The globals come first, then the
//! program instances' variables, an array's elements in row-major order; in an instance its base
//! part first, then its own nested instances, then its own `FB_init` and after-init methods. The
//! inputs after `bInitRetains` and `bInCopyCode` take the arguments of the instance's
//! declaration, `com1 : serialdevice(iCOMnum := 1)`, which the build works out as it works out
//! initial values. The members the declaration initializes, `fb : FB := (in1 := 99)`, hold their
//! type's initial values while `FB_init` runs and take the declaration's values right after it.

use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;

use super::front::Front;
use super::lower::Need;
use super::model::Model;
use super::types::{rust_field, Elem, Kind, Ty};
use super::NativeError;
use crate::program_model::{Expr, InitArg};
use crate::value::Value;

/// The method CODESYS calls implicitly.
const FB_INIT: &str = "FB_init";

/// The calls a start or an online change makes for the instances of one top-level variable, a
/// global or a program instance's variable (specification 44 §6.7, §12.3).
pub(super) struct VariableCalls {
    /// The variable's canonical name in the symbol table (§5.7): `NAME`, `INSTANCE.NAME`.
    pub(super) canonical: String,
    /// Rust statements on `cx` and `w`.
    pub(super) statements: Vec<String>,
}

/// The calls of `FB_init` the initializations make, and the methods they need.
#[derive(Default)]
pub(super) struct FbInit {
    /// The variables with calls, in the initialization's order; their statements pass `retains`
    /// as `bInitRetains` and `copy` as `bInCopyCode`.
    pub(super) variables: Vec<VariableCalls>,
    pub(super) needs: Vec<Need>,
    /// The statements of the variable the walk is in.
    statements: Vec<String>,
}

/// The calls of `FB_init` for every instance of `model`, none outside the CODESYS profiles.
///
/// # Errors
///
/// [`NativeError`] for an `FB_init` whose first inputs are not the two `BOOL` ones CODESYS passes,
/// or with an output, and for an argument the build does not work out (NX-09).
pub(super) fn calls(front: &Front, model: &mut Model<'_>) -> Result<FbInit, NativeError> {
    let mut calls = FbInit::default();
    if !crate::harness::dialect_of(model.conversions).is_codesys() {
        return Ok(calls);
    }
    let declared: Vec<_> = front
        .lowered
        .globals
        .iter()
        .chain(
            front
                .lowered
                .configuration
                .iter()
                .flat_map(|configuration| configuration.globals.iter()),
        )
        .collect();
    let globals: Vec<_> = model
        .globals
        .iter()
        .map(|global| (global.name.clone(), global.ty, global.retained))
        .collect();
    for (name, ty, retained) in globals {
        let area = if retained { "r" } else { "g" };
        let declaration = declared
            .iter()
            .find(|declared| declared.name.eq_ignore_ascii_case(&name));
        calls.instance(
            model,
            &format!("w.{area}.{}", rust_field(&name)),
            ty,
            Declaration {
                args: declaration.map_or(&[][..], |declared| declared.init_args.as_slice()),
                initializer: declaration.and_then(|declared| declared.initializer.as_ref()),
                retained,
            },
            0,
        )?;
        calls.close(name.to_ascii_uppercase());
    }
    let mut variables = Vec::new();
    for instance in &model.instances {
        let program = &model.program_types[instance.program];
        for var in program.def.vars.iter().filter(|var| !var.external) {
            let Some((member, retained)) = program.member(&var.name) else {
                continue;
            };
            variables.push((
                format!(
                    "w.{}.{}.{}",
                    if retained { "r" } else { "p" },
                    rust_field(&instance.name),
                    member.field()
                ),
                member.ty,
                var.clone(),
                retained,
                format!(
                    "{}.{}",
                    instance.name.to_ascii_uppercase(),
                    var.name.to_ascii_uppercase()
                ),
            ));
        }
    }
    for (place, ty, var, retained, canonical) in variables {
        calls.instance(
            model,
            &place,
            ty,
            Declaration {
                args: &var.init_args,
                initializer: var.initializer.as_ref(),
                retained,
            },
            0,
        )?;
        calls.close(canonical);
    }
    Ok(calls)
}

/// What an instance's declaration gives it: the `FB_init` arguments in parentheses after its type,
/// its initial value, and whether a warm restart keeps it whole.
#[derive(Clone, Copy)]
struct Declaration<'d> {
    args: &'d [InitArg],
    initializer: Option<&'d Expr>,
    retained: bool,
}

impl Declaration<'_> {
    /// A value that holds instances but declares none of them itself: an element of an array, a
    /// member of a structure.
    const NONE: Declaration<'static> = Declaration {
        args: &[],
        initializer: None,
        retained: false,
    };
}

impl FbInit {
    /// Ends the walk of the variable `canonical`: its statements, when it has any, become its calls.
    fn close(&mut self, canonical: String) {
        let statements = std::mem::take(&mut self.statements);
        if !statements.is_empty() {
            self.variables.push(VariableCalls {
                canonical,
                statements,
            });
        }
    }

    /// The calls for the value at `place` of type `ty`, as `declaration` declares it: an instance,
    /// or what holds instances.
    fn instance(
        &mut self,
        model: &mut Model<'_>,
        place: &str,
        ty: Ty,
        declaration: Declaration<'_>,
        depth: usize,
    ) -> Result<(), NativeError> {
        match model.types.info(ty).kind.clone() {
            Kind::Fb {
                def, members, base, ..
            } => {
                // CODESYS gives the members the declaration initializes their values after the
                // instance's `FB_init` (its help, Example 259), so they hold their type's initial
                // values until then.
                let (before, after) = match declaration.initializer {
                    Some(initializer) => {
                        declared_values(model, place, ty, initializer, declaration.retained)?
                    }
                    None => (Vec::new(), Vec::new()),
                };
                self.statements.extend(before);
                // The base part's `FB_init` takes the same arguments: a derived `FB_init` repeats
                // its base's inputs.
                if let Some(base) = base {
                    let part = Declaration {
                        initializer: None,
                        ..declaration
                    };
                    self.instance(model, &format!("{place}.__BASE"), base, part, depth)?;
                }
                // The variables it inherits are its base part's; each of its own takes the
                // arguments and the initial value of its declaration in the type.
                let owner = model.types.owner(&def);
                for member in members.iter().filter(|member| member.via.is_empty()) {
                    let var = owner.and_then(|owner| {
                        owner
                            .vars()
                            .iter()
                            .find(|var| var.name.eq_ignore_ascii_case(&member.name))
                    });
                    let nested = Declaration {
                        args: var.map_or(&[][..], |var| var.init_args.as_slice()),
                        initializer: var.and_then(|var| var.initializer.as_ref()),
                        retained: declaration.retained || var.is_some_and(kept_at_warm_restart),
                    };
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        nested,
                        depth,
                    )?;
                }
                self.own(model, place, &def, declaration.args, after)
            }
            Kind::Struct { members } => {
                for member in members {
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        Declaration::NONE,
                        depth,
                    )?;
                }
                Ok(())
            }
            Kind::Array { elem, dims } => self.elements(model, place, elem, &dims, depth),
            _ => Ok(()),
        }
    }

    /// The calls for each element of the array at `place`, a loop per dimension, in row-major
    /// order; none when its elements hold no instance that declares `FB_init`.
    fn elements(
        &mut self,
        model: &mut Model<'_>,
        place: &str,
        elem: Ty,
        dims: &[(i64, i64)],
        depth: usize,
    ) -> Result<(), NativeError> {
        let Some((_, inner)) = dims.split_first() else {
            return self.instance(model, place, elem, Declaration::NONE, depth);
        };
        let item = format!("e{depth}");
        let outer = std::mem::take(&mut self.statements);
        self.elements(model, &item, elem, inner, depth + 1)?;
        let body = std::mem::replace(&mut self.statements, outer);
        if !body.is_empty() {
            self.statements
                .push(format!("for {item} in {place}.iter() {{"));
            self.statements
                .extend(body.into_iter().map(|line| format!("    {line}")));
            self.statements.push("}".into());
        }
        Ok(())
    }

    /// The calls of the `FB_init` the type `def` declares itself, with the arguments `args` of the
    /// instance's declaration, then the writes `declared` of the values the declaration gives,
    /// then the calls of its methods marked `{attribute 'call_after_init'}`, on its part at `place`.
    fn own(
        &mut self,
        model: &Model<'_>,
        place: &str,
        def: &str,
        args: &[InitArg],
        declared: Vec<String>,
    ) -> Result<(), NativeError> {
        let Some(owner) = model.types.owner(def) else {
            self.statements.extend(declared);
            return Ok(());
        };
        self.fb_init(model, owner, place, args)?;
        self.statements.extend(declared);
        self.after_init(model, owner, place)
    }

    /// The method CODESYS calls after an instance's initialization (the vendor's help for
    /// `call_after_init`): each method marked so, when its block is marked too, with no inputs
    /// (the vendor refuses one) and a `BOOL` or no result, called after the instance's `FB_init`.
    fn after_init(
        &mut self,
        model: &Model<'_>,
        owner: super::classes::Owner<'_>,
        place: &str,
    ) -> Result<(), NativeError> {
        if !owner.calls_after_init() {
            return Ok(());
        }
        let registry = model.types.registry();
        for method in owner
            .methods()
            .iter()
            .filter(|method| method.call_after_init)
        {
            let inputs = method.params.iter().any(|param| {
                !param.name.eq_ignore_ascii_case("EN") && !param.name.eq_ignore_ascii_case("ENO")
            });
            let result = method.return_type.is_some_and(|ty| {
                !matches!(
                    registry.get(ty),
                    Some(trust_hir::Type::Bool | trust_hir::Type::Void)
                )
            });
            if inputs || result {
                return Err(NativeError::unsupported(format!(
                    "{}.{} with {}: a method called after the initialization takes no inputs",
                    owner.name(),
                    method.name,
                    if inputs {
                        "inputs"
                    } else {
                        "a result other than BOOL"
                    }
                )));
            }
            Need::Method(owner.name().clone(), method.name.clone()).add_to(&mut self.needs);
            self.statements.push(format!(
                "{}(cx, w, &{place})?;",
                super::emit::method_function(owner, method)
            ));
        }
        Ok(())
    }

    /// The call of the `FB_init` the type `def` declares itself, on its part at `place`, with the
    /// arguments `args` of the instance's declaration.
    fn fb_init(
        &mut self,
        model: &Model<'_>,
        owner: super::classes::Owner<'_>,
        place: &str,
        args: &[InitArg],
    ) -> Result<(), NativeError> {
        let Some(method) = owner
            .methods()
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(FB_INIT))
        else {
            return Ok(());
        };
        let registry = model.types.registry();
        let is_bool = |ty| matches!(registry.get(ty), Some(trust_hir::Type::Bool));
        let params: Vec<_> = method
            .params
            .iter()
            .filter(|param| {
                !param.name.eq_ignore_ascii_case("EN") && !param.name.eq_ignore_ascii_case("ENO")
            })
            .collect();
        let refused = if params
            .iter()
            .any(|param| param.direction != ParamDirection::In)
        {
            Some("an output or an in-out")
        } else if params.len() < 2 || !params[..2].iter().all(|param| is_bool(param.type_id)) {
            Some("other first inputs than the BOOL ones bInitRetains and bInCopyCode")
        } else if method.return_type.is_some_and(|ty| {
            !is_bool(ty) && !matches!(registry.get(ty), Some(trust_hir::Type::Void))
        }) {
            Some("a result other than BOOL")
        } else {
            None
        };
        if let Some(what) = refused {
            return Err(NativeError::unsupported(format!(
                "{}.{} with {what}",
                owner.name(),
                method.name
            )));
        }
        // The inputs after the two the runtime passes take the declaration's arguments, by name
        // or in order, else their own initial value, else their type's default.
        let mut arguments = String::new();
        let mut positional = args.iter().filter(|arg| arg.name.is_none());
        for param in &params[2..] {
            let given = args
                .iter()
                .find(|arg| {
                    arg.name
                        .as_ref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&param.name))
                })
                .or_else(|| positional.next());
            let value = start_value(
                model,
                param.type_id,
                given.map(|arg| &arg.value).or(param.default.as_ref()),
            )
            .map_err(|error| {
                NativeError::unsupported(format!(
                    "{}.{}'s input {} from the instance's declaration ({error})",
                    owner.name(),
                    method.name,
                    param.name
                ))
            })?;
            let literal = literal(model, param.type_id, &value).ok_or_else(|| {
                NativeError::unsupported(format!(
                    "{}.{}'s input {}, of a type that is no scalar, from the instance's \
                     declaration",
                    owner.name(),
                    method.name,
                    param.name
                ))
            })?;
            arguments.push_str(", ");
            arguments.push_str(&literal);
        }
        Need::Method(owner.name().clone(), method.name.clone()).add_to(&mut self.needs);
        self.statements.push(format!(
            "{}(cx, w, &{place}, retains, copy{arguments})?;",
            super::emit::method_function(owner, method)
        ));
        Ok(())
    }
}

/// The writes around the `FB_init` of the instance at `place` of type `ty` that give the members its
/// declaration's initial value `initializer` names CODESYS's order (specification 10 §4.8): before
/// it, each member's initial value in the type; after it, the declaration's. None when the type
/// declares no `FB_init`; none for a member that holds an instance, whose own declaration counts,
/// or for a member a warm restart keeps, of a `retained` instance or declared `RETAIN` in the type,
/// which holds the declaration's value from the start, as Beckhoff orders retained variables.
fn declared_values(
    model: &mut Model<'_>,
    place: &str,
    ty: Ty,
    initializer: &Expr,
    retained: bool,
) -> Result<(Vec<String>, Vec<String>), NativeError> {
    let (Expr::StructInitializer(fields), Kind::Fb { def, members, .. }) =
        (initializer, model.types.info(ty).kind.clone())
    else {
        return Ok((Vec::new(), Vec::new()));
    };
    if !declares_fb_init(model, &def) {
        return Ok((Vec::new(), Vec::new()));
    }
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for (name, value) in fields {
        let Some(member) = members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(name))
        else {
            continue;
        };
        let Some(own) = member_declaration(model, &def, &member.name) else {
            continue;
        };
        if retained || own.kept || model.types.holds_instance(member.ty) {
            continue;
        }
        let failed = |error: String| {
            NativeError::unsupported(format!(
                "the initial value of {name} from the declaration of an instance of {def} ({error})"
            ))
        };
        let typed = start_value(model, own.type_id, own.initial.as_ref()).map_err(failed)?;
        let given = start_value(model, own.type_id, Some(value)).map_err(failed)?;
        let field = format!("{place}.{}", member.field());
        write(model, &field, member.ty, &typed, &mut before)?;
        write(model, &field, member.ty, &given, &mut after)?;
    }
    Ok((before, after))
}

/// A member's declaration in its type: trust-hir's type, its initial value, and whether a warm
/// restart keeps it in every instance.
struct MemberDeclaration {
    type_id: TypeId,
    initial: Option<Expr>,
    kept: bool,
}

/// The declaration of the variable `name` that the function block or class `def`, or a type it
/// extends, declares: a `VAR` or `VAR_STAT`, an input or an output.
fn member_declaration(model: &Model<'_>, def: &str, name: &str) -> Option<MemberDeclaration> {
    let mut owner = model.types.owner(def);
    while let Some(current) = owner {
        if let Some(var) = current
            .vars()
            .iter()
            .find(|var| var.name.eq_ignore_ascii_case(name))
        {
            return Some(MemberDeclaration {
                type_id: var.type_id,
                initial: var.initializer.clone(),
                kept: kept_at_warm_restart(var),
            });
        }
        if let Some(param) = current
            .params()
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(name))
        {
            return Some(MemberDeclaration {
                type_id: param.type_id,
                initial: param.default.clone(),
                kept: matches!(
                    param.retain,
                    crate::RetainPolicy::Retain | crate::RetainPolicy::Persistent
                ),
            });
        }
        owner = model.types.defs.base(current);
    }
    None
}

/// Whether a warm restart keeps the variable `var` in every instance: it is declared `RETAIN` or
/// `PERSISTENT`.
fn kept_at_warm_restart(var: &crate::program_model::VarDef) -> bool {
    matches!(
        var.retain,
        crate::RetainPolicy::Retain | crate::RetainPolicy::Persistent
    )
}

/// Whether the function block or class `def`, or a type it extends, declares `FB_init`.
fn declares_fb_init(model: &Model<'_>, def: &str) -> bool {
    let mut owner = model.types.owner(def);
    while let Some(current) = owner {
        if current
            .methods()
            .iter()
            .any(|method| method.name.eq_ignore_ascii_case(FB_INIT))
        {
            return true;
        }
        owner = model.types.defs.base(current);
    }
    false
}

/// The statements that give the variable at `place` of type `ty` the value `value`.
fn write(
    model: &mut Model<'_>,
    place: &str,
    ty: Ty,
    value: &Value,
    out: &mut Vec<String>,
) -> Result<(), NativeError> {
    let bytes = super::init::value_bytes(model, value, ty)?;
    let rust = model.types.info(ty).rust.clone();
    // Zero first: the value is written from its parts that are not zero.
    out.push(format!(
        "{{ let zero: {rust} = <{rust} as Plain>::ZEROED; Assign::assign(&{place}, &zero); }}"
    ));
    super::frame_init::statements(model, place, ty, &bytes, 0, out)
}

/// The value an input of trust-hir's type `type_id` starts at: `expr`, worked out by the build as
/// an initial value is, else its type's default.
fn start_value(model: &Model<'_>, type_id: TypeId, expr: Option<&Expr>) -> Result<Value, String> {
    let builder = model.builder;
    match expr {
        Some(expr) => crate::harness::initializer::evaluate_initializer(
            builder.storage(),
            builder.registry(),
            builder.initializer_catalog(),
            &builder.profile(),
            None,
            builder.stdlib(),
            expr,
            type_id,
        ),
        None => crate::harness::initializer::default_value_for_type_id(
            builder.storage(),
            builder.registry(),
            builder.initializer_catalog(),
            &builder.profile(),
            None,
            builder.stdlib(),
            type_id,
        ),
    }
    .map_err(|error| error.to_string())
}

/// The Rust literal of `value` for an input of trust-hir's type `type_id`, when that is a scalar:
/// an elementary type, an enumeration or a subrange, which a call passes by value.
fn literal(model: &Model<'_>, type_id: TypeId, value: &Value) -> Option<String> {
    let registry = model.types.registry();
    let mut ty = registry.get(type_id)?;
    while let trust_hir::Type::Alias { target, .. } = ty {
        ty = registry.get(*target)?;
    }
    let elem = match ty {
        trust_hir::Type::Enum { base, .. } | trust_hir::Type::Subrange { base, .. } => {
            Elem::of(registry.get(*base)?)?
        }
        other => Elem::of(other)?,
    };
    let bytes = super::init::scalar(elem, value)?;
    Some(match elem {
        Elem::Bool => (bytes[0] != 0).to_string(),
        _ => super::frame_init::scalar(elem, &bytes),
    })
}
