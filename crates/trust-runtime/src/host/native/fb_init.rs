//! CODESYS's implicit `FB_init` (specification 10 §4.8; specification 44 §6.7): under the CODESYS
//! profiles the cold-start and the warm-start initializations call `FB_init` of every instance
//! whose type declares it, after the `REF` initializers. The globals come first, then the program
//! instances' variables, an array's elements in row-major order; in an instance its base part
//! first, then its own nested instances, then its own `FB_init`.

use trust_hir::symbols::ParamDirection;

use super::lower::Need;
use super::model::Model;
use super::types::{rust_field, Kind, Ty};
use super::NativeError;

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
/// [`NativeError`] for an `FB_init` whose inputs are not the two `BOOL` ones CODESYS passes
/// (NX-09).
pub(super) fn calls(model: &Model<'_>) -> Result<FbInit, NativeError> {
    let mut calls = FbInit::default();
    if !crate::harness::dialect_of(model.conversions).is_codesys() {
        return Ok(calls);
    }
    for global in &model.globals {
        let area = if global.retained { "r" } else { "g" };
        calls.instance(
            model,
            &format!("w.{area}.{}", rust_field(&global.name)),
            global.ty,
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
                0,
            )?;
        }
    }
    Ok(calls)
}

impl FbInit {
    /// The calls for the value at `place` of type `ty`: an instance, or what holds instances.
    fn instance(
        &mut self,
        model: &Model<'_>,
        place: &str,
        ty: Ty,
        depth: usize,
    ) -> Result<(), NativeError> {
        match &model.types.info(ty).kind {
            Kind::Fb {
                def, members, base, ..
            } => {
                if let Some(base) = base {
                    self.instance(model, &format!("{place}.__BASE"), *base, depth)?;
                }
                // The variables it inherits are its base part's.
                for member in members.iter().filter(|member| member.via.is_empty()) {
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        depth,
                    )?;
                }
                self.own(model, place, def)
            }
            Kind::Struct { members } => {
                for member in members {
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
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
            return self.instance(model, place, elem, depth);
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

    /// The call of the `FB_init` the type `def` declares itself, on its part at `place`.
    fn own(&mut self, model: &Model<'_>, place: &str, def: &str) -> Result<(), NativeError> {
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
        } else if params.len() > 2 {
            Some("inputs beyond bInitRetains and bInCopyCode, which the instance's declaration gives")
        } else if params.len() < 2 || !params.iter().all(|param| is_bool(param.type_id)) {
            Some("other inputs than the BOOL ones bInitRetains and bInCopyCode")
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
        Need::Method(owner.name().clone(), method.name.clone()).add_to(&mut self.needs);
        self.statements.push(format!(
            "{}(cx, w, &{place}, retains, false)?;",
            super::emit::method_function(owner, method)
        ));
        Ok(())
    }
}
