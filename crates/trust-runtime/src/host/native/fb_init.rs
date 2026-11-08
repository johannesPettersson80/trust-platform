//! CODESYS's implicit `FB_init` (specification 10 §4.8; specification 44 §6.7): under the CODESYS
//! profiles the cold-start and the warm-start initializations call `FB_init` of every instance
//! whose type declares it, after the `REF` initializers. The globals come first, then the program
//! instances' variables, an array's elements in row-major order; in an instance its base part
//! first, then its own nested instances, then its own `FB_init`. The inputs after `bInitRetains`
//! and `bInCopyCode` take the arguments of the instance's declaration, `com1 :
//! serialdevice(iCOMnum := 1)`, which the build works out as it works out initial values.

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

/// The calls of `FB_init` the initializations make, and the methods they need.
#[derive(Default)]
pub(super) struct FbInit {
    /// Rust statements on `cx` and `w`, passing `retains` as `bInitRetains`.
    pub(super) statements: Vec<String>,
    pub(super) needs: Vec<Need>,
}

/// The calls of `FB_init` for every instance of `model`, none outside the CODESYS profiles.
///
/// # Errors
///
/// [`NativeError`] for an `FB_init` whose first inputs are not the two `BOOL` ones CODESYS passes,
/// or with an output, and for an argument the build does not work out (NX-09).
pub(super) fn calls(front: &Front, model: &Model<'_>) -> Result<FbInit, NativeError> {
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
    for global in &model.globals {
        let area = if global.retained { "r" } else { "g" };
        let args = declared
            .iter()
            .find(|declared| declared.name.eq_ignore_ascii_case(&global.name))
            .map_or(&[][..], |declared| declared.init_args.as_slice());
        calls.instance(
            model,
            &format!("w.{area}.{}", rust_field(&global.name)),
            global.ty,
            args,
            0,
        )?;
    }
    for instance in &model.instances {
        let program = &model.program_types[instance.program];
        for var in program.def.vars.iter().filter(|var| !var.external) {
            let Some((member, retained)) = program.member(&var.name) else {
                continue;
            };
            let area = if retained { "r" } else { "p" };
            calls.instance(
                model,
                &format!("w.{area}.{}.{}", rust_field(&instance.name), member.field()),
                member.ty,
                &var.init_args,
                0,
            )?;
        }
    }
    Ok(calls)
}

impl FbInit {
    /// The calls for the value at `place` of type `ty`, declared with the `FB_init` arguments
    /// `args`: an instance, or what holds instances.
    fn instance(
        &mut self,
        model: &Model<'_>,
        place: &str,
        ty: Ty,
        args: &[InitArg],
        depth: usize,
    ) -> Result<(), NativeError> {
        match &model.types.info(ty).kind {
            Kind::Fb {
                def, members, base, ..
            } => {
                // The base part's `FB_init` takes the same arguments: a derived `FB_init` repeats
                // its base's inputs.
                if let Some(base) = base {
                    self.instance(model, &format!("{place}.__BASE"), *base, args, depth)?;
                }
                // The variables it inherits are its base part's; each of its own takes the
                // arguments of its declaration in the type.
                let owner = model.types.owner(def);
                for member in members.iter().filter(|member| member.via.is_empty()) {
                    let member_args = owner
                        .and_then(|owner| {
                            owner
                                .vars()
                                .iter()
                                .find(|var| var.name.eq_ignore_ascii_case(&member.name))
                        })
                        .map_or(&[][..], |var| var.init_args.as_slice());
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        member_args,
                        depth,
                    )?;
                }
                self.own(model, place, def, args)
            }
            Kind::Struct { members } => {
                for member in members {
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        &[],
                        depth,
                    )?;
                }
                Ok(())
            }
            Kind::Array { elem, dims } => self.elements(model, place, *elem, dims, depth),
            _ => Ok(()),
        }
    }

    /// The calls for each element of the array at `place`, a loop per dimension, in row-major
    /// order; none when its elements hold no instance that declares `FB_init`.
    fn elements(
        &mut self,
        model: &Model<'_>,
        place: &str,
        elem: Ty,
        dims: &[(i64, i64)],
        depth: usize,
    ) -> Result<(), NativeError> {
        let Some((_, inner)) = dims.split_first() else {
            return self.instance(model, place, elem, &[], depth);
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

    /// The call of the `FB_init` the type `def` declares itself, on its part at `place`, with the
    /// arguments `args` of the instance's declaration.
    fn own(
        &mut self,
        model: &Model<'_>,
        place: &str,
        def: &str,
        args: &[InitArg],
    ) -> Result<(), NativeError> {
        let Some(owner) = model.types.owner(def) else {
            return Ok(());
        };
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
            "{}(cx, w, &{place}, retains, false{arguments})?;",
            super::emit::method_function(owner, method)
        ));
        Ok(())
    }
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
