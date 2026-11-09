//! CODESYS's implicit `FB_reinit` (specification 10 §4.8; specification 44 §12.3): under the CODESYS
//! profiles an online change calls `FB_reinit()` of every instance of a variable it copied, after
//! the carry, in `FB_init`'s order: the globals, then the program instances' variables; an array's
//! elements in row-major order; in an instance its nested instances first, its base part's before
//! its own, then the instance's own call. The call is the one `inst.FB_reinit()` reaches: the
//! method the instance's type declares, else the nearest base's, on that base's part; a base's
//! `FB_reinit` runs only when the block calls it, as CODESYS leaves it to the block.

use super::fb_init::VariableCalls;
use super::lower::Need;
use super::model::Model;
use super::types::{rust_field, Kind, Member, Ty};
use super::NativeError;

/// The method CODESYS calls implicitly after an online change copied an instance.
const FB_REINIT: &str = "FB_reinit";

/// The calls of `FB_reinit` an online change makes, and the methods they need.
#[derive(Default)]
pub(super) struct FbReinit {
    /// The variables with calls, in `FB_init`'s order.
    pub(super) variables: Vec<VariableCalls>,
    pub(super) needs: Vec<Need>,
    /// The statements of the variable the walk is in.
    statements: Vec<String>,
}

/// The calls of `FB_reinit` for every instance of `model`, none outside the CODESYS profiles.
///
/// # Errors
///
/// [`NativeError`] for an `FB_reinit` with an input, an output or a result other than `BOOL`
/// (NX-09).
pub(super) fn calls(model: &Model<'_>) -> Result<FbReinit, NativeError> {
    let mut calls = FbReinit::default();
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
        calls.close(global.name.to_ascii_uppercase());
    }
    for instance in &model.instances {
        let program = &model.program_types[instance.program];
        for var in program.def.vars.iter().filter(|var| !var.external) {
            let Some((member, retained)) = program.member(&var.name) else {
                continue;
            };
            let place = format!(
                "w.{}.{}.{}",
                if retained { "r" } else { "p" },
                rust_field(&instance.name),
                member.field()
            );
            calls.instance(model, &place, member.ty, 0)?;
            calls.close(format!(
                "{}.{}",
                instance.name.to_ascii_uppercase(),
                var.name.to_ascii_uppercase()
            ));
        }
    }
    Ok(calls)
}

impl FbReinit {
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

    /// The calls for the value at `place` of type `ty`: an instance, or what holds instances.
    fn instance(
        &mut self,
        model: &Model<'_>,
        place: &str,
        ty: Ty,
        depth: usize,
    ) -> Result<(), NativeError> {
        match model.types.info(ty).kind.clone() {
            Kind::Fb { members, base, .. } => {
                self.parts(model, place, &members, base, depth)?;
                self.reinit(model, place, ty)
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
            Kind::Array { elem, dims } => self.elements(model, place, elem, &dims, depth),
            _ => Ok(()),
        }
    }

    /// The calls for the instances nested in the part at `place` whose own members are `members`:
    /// its base part's first, then its own, in declaration order.
    fn parts(
        &mut self,
        model: &Model<'_>,
        place: &str,
        members: &[Member],
        base: Option<Ty>,
        depth: usize,
    ) -> Result<(), NativeError> {
        if let Some(base) = base
            && let Kind::Fb {
                members: inherited,
                base: further,
                ..
            } = model.types.info(base).kind.clone()
        {
            self.parts(
                model,
                &format!("{place}.__BASE"),
                &inherited,
                further,
                depth,
            )?;
        }
        for member in members.iter().filter(|member| member.via.is_empty()) {
            self.instance(
                model,
                &format!("{place}.{}", member.field()),
                member.ty,
                depth,
            )?;
        }
        Ok(())
    }

    /// The calls for each element of the array at `place`, a loop per dimension, in row-major
    /// order; none when its elements hold no instance that has `FB_reinit`.
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

    /// The call of the `FB_reinit` that `inst.FB_reinit()` reaches for the instance at `place` of
    /// type `ty`: the one its type declares, else the nearest base's, on that base's part.
    fn reinit(&mut self, model: &Model<'_>, place: &str, ty: Ty) -> Result<(), NativeError> {
        let mut part = place.to_string();
        let mut current = ty;
        loop {
            let Kind::Fb { def, base, .. } = model.types.info(current).kind.clone() else {
                return Ok(());
            };
            if let Some(owner) = model.types.owner(&def)
                && let Some(method) = owner
                    .methods()
                    .iter()
                    .find(|method| method.name.eq_ignore_ascii_case(FB_REINIT))
            {
                let registry = model.types.registry();
                let inputs = method.params.iter().any(|param| {
                    !param.name.eq_ignore_ascii_case("EN")
                        && !param.name.eq_ignore_ascii_case("ENO")
                });
                let result = method.return_type.is_some_and(|ty| {
                    !matches!(
                        registry.get(ty),
                        Some(trust_hir::Type::Bool | trust_hir::Type::Void)
                    )
                });
                if inputs || result {
                    return Err(NativeError::unsupported(format!(
                        "{}.{} with {}: FB_reinit takes no input and returns a BOOL or nothing",
                        owner.name(),
                        method.name,
                        if inputs {
                            "a parameter"
                        } else {
                            "another result"
                        }
                    )));
                }
                Need::Method(owner.name().clone(), method.name.clone()).add_to(&mut self.needs);
                self.statements.push(format!(
                    "{}(cx, w, &{part})?;",
                    super::emit::method_function(owner, method)
                ));
                return Ok(());
            }
            let Some(base) = base else {
                return Ok(());
            };
            part = format!("{part}.__BASE");
            current = base;
        }
    }
}
