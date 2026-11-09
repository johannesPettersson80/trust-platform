//! CODESYS's implicit `FB_exit` (specification 10 §4.8; specification 44 §6.7): under the CODESYS
//! profiles the exit that leaves a program's instances before a restart or a download calls
//! `FB_exit(bInCopyCode := FALSE)` of every instance whose type declares it, in the reverse of
//! `FB_init`'s order: the program instances in reverse configuration order, each with its variables
//! in reverse declaration order, then the globals in reverse declaration order; an array's
//! elements in reverse row-major order; in an instance its own `FB_exit` first, then its own nested
//! instances in reverse declaration order, then its base part. A declaration marked
//! `{attribute 'no-exit'}` leaves out the `FB_exit` of the instances it declares, their base parts'
//! too; an instance nested in one of them keeps the mark of its own declaration.

use trust_hir::symbols::ParamDirection;

use super::fb_init::VariableCalls;
use super::front::Front;
use super::lower::Need;
use super::model::Model;
use super::types::{rust_field, Kind, Ty};
use super::NativeError;

/// The method CODESYS calls implicitly when an instance leaves.
const FB_EXIT: &str = "FB_exit";

/// The calls of `FB_exit` the exit makes, and the methods they need.
#[derive(Default)]
pub(super) struct FbExit {
    /// The variables with calls, in the exit's order; their statements pass `copy` as
    /// `bInCopyCode`.
    pub(super) variables: Vec<VariableCalls>,
    pub(super) needs: Vec<Need>,
    /// The statements of the variable the walk is in.
    statements: Vec<String>,
}

/// The calls of `FB_exit` for every instance of `model`, none outside the CODESYS profiles.
///
/// # Errors
///
/// [`NativeError`] for an `FB_exit` whose inputs are not the one `BOOL` input CODESYS passes, or
/// with an output or a result other than `BOOL` (NX-09).
pub(super) fn calls(front: &Front, model: &Model<'_>) -> Result<FbExit, NativeError> {
    let mut calls = FbExit::default();
    if !crate::harness::dialect_of(model.conversions).is_codesys() {
        return Ok(calls);
    }
    for instance in model.instances.iter().rev() {
        let program = &model.program_types[instance.program];
        for var in program.def.vars.iter().rev().filter(|var| !var.external) {
            let Some((member, retained)) = program.member(&var.name) else {
                continue;
            };
            let place = format!(
                "w.{}.{}.{}",
                if retained { "r" } else { "p" },
                rust_field(&instance.name),
                member.field()
            );
            calls.instance(model, &place, member.ty, var.no_exit, 0)?;
            calls.close(format!(
                "{}.{}",
                instance.name.to_ascii_uppercase(),
                var.name.to_ascii_uppercase()
            ));
        }
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
    for global in model.globals.iter().rev() {
        let area = if global.retained { "r" } else { "g" };
        let no_exit = declared
            .iter()
            .find(|declared| declared.name.eq_ignore_ascii_case(&global.name))
            .is_some_and(|declared| declared.no_exit);
        calls.instance(
            model,
            &format!("w.{area}.{}", rust_field(&global.name)),
            global.ty,
            no_exit,
            0,
        )?;
        calls.close(global.name.to_ascii_uppercase());
    }
    Ok(calls)
}

impl FbExit {
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
    /// `no_exit` leaves out the `FB_exit` of the instances its declaration declares.
    fn instance(
        &mut self,
        model: &Model<'_>,
        place: &str,
        ty: Ty,
        no_exit: bool,
        depth: usize,
    ) -> Result<(), NativeError> {
        match model.types.info(ty).kind.clone() {
            Kind::Fb {
                def, members, base, ..
            } => {
                if !no_exit {
                    self.own(model, place, &def)?;
                }
                // Each of its own nested instances leaves as its declaration in the type says.
                let owner = model.types.owner(&def);
                for member in members.iter().rev().filter(|member| member.via.is_empty()) {
                    let marked = owner
                        .and_then(|owner| {
                            owner
                                .vars()
                                .iter()
                                .find(|var| var.name.eq_ignore_ascii_case(&member.name))
                        })
                        .is_some_and(|var| var.no_exit);
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        marked,
                        depth,
                    )?;
                }
                // The base part is the same instance, so the declaration's mark holds for it.
                if let Some(base) = base {
                    self.instance(model, &format!("{place}.__BASE"), base, no_exit, depth)?;
                }
                Ok(())
            }
            Kind::Struct { members } => {
                for member in members.iter().rev() {
                    self.instance(
                        model,
                        &format!("{place}.{}", member.field()),
                        member.ty,
                        no_exit,
                        depth,
                    )?;
                }
                Ok(())
            }
            Kind::Array { elem, dims } => self.elements(model, place, elem, &dims, no_exit, depth),
            _ => Ok(()),
        }
    }

    /// The calls for each element of the array at `place`, a loop per dimension, in reverse
    /// row-major order; none when its elements hold no instance that declares `FB_exit`.
    fn elements(
        &mut self,
        model: &Model<'_>,
        place: &str,
        elem: Ty,
        dims: &[(i64, i64)],
        no_exit: bool,
        depth: usize,
    ) -> Result<(), NativeError> {
        let Some((_, inner)) = dims.split_first() else {
            return self.instance(model, place, elem, no_exit, depth);
        };
        let item = format!("e{depth}");
        let outer = std::mem::take(&mut self.statements);
        self.elements(model, &item, elem, inner, no_exit, depth + 1)?;
        let body = std::mem::replace(&mut self.statements, outer);
        if !body.is_empty() {
            self.statements
                .push(format!("for {item} in {place}.iter().rev() {{"));
            self.statements
                .extend(body.into_iter().map(|line| format!("    {line}")));
            self.statements.push("}".into());
        }
        Ok(())
    }

    /// The call of the `FB_exit` the type `def` declares itself, on its part at `place`, with
    /// `bInCopyCode := copy`: FALSE everywhere but for an instance an online change copies.
    fn own(&mut self, model: &Model<'_>, place: &str, def: &str) -> Result<(), NativeError> {
        let Some(owner) = model.types.owner(def) else {
            return Ok(());
        };
        let Some(method) = owner
            .methods()
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(FB_EXIT))
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
        } else if params.len() != 1 || !is_bool(params[0].type_id) {
            Some("other inputs than the BOOL one bInCopyCode")
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
            "{}(cx, w, &{place}, copy)?;",
            super::emit::method_function(owner, method)
        ));
        Ok(())
    }
}
