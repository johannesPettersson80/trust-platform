//! `EN` and `ENO` of a call (IEC 61131-3 §6.6.1.4 and Table 18; specification 10 §7.5): `EN`, when
//! the call names it, is evaluated before every other actual; a call it disables evaluates no other
//! actual and runs nothing, a value-producing one gives its declared type's default, and a
//! connected `ENO` gets `FALSE`. An enabled call runs as it would without `EN`; `ENO` starts
//! `TRUE` in the callee, which may set it `FALSE`, and reaches a connected `ENO` like any output.

use trust_syntax::syntax::SyntaxNode;

use super::call::{Arg, Called, Want};
use super::{is_scalar, Lowerer, Place};
use crate::native::types::Elem;
use crate::native::NativeError;

/// The call's `EN := value`, when it names one, and its other arguments.
pub(super) fn split_en(mut args: Vec<Arg>) -> (Option<Arg>, Vec<Arg>) {
    let position = args.iter().position(|arg| {
        !arg.output
            && arg
                .name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("EN"))
    });
    let en = position.map(|position| args.remove(position));
    (en, args)
}

impl Lowerer<'_, '_> {
    /// The call that `call` lowers with `args`, run only when its `EN`, if it names one, is TRUE;
    /// `ret` is the callee's return type, of which a disabled call gives the declared default.
    pub(super) fn enabled<F>(
        &mut self,
        node: &SyntaxNode,
        args: Vec<Arg>,
        ret: Option<trust_hir::TypeId>,
        want: Want,
        call: F,
    ) -> Result<Called, NativeError>
    where
        F: FnOnce(&mut Self, Vec<Arg>, Want) -> Result<Called, NativeError>,
    {
        let (en, args) = split_en(args);
        let Some(en) = en else {
            return call(self, args, want);
        };
        let eno = args
            .iter()
            .find(|arg| {
                arg.output
                    && arg
                        .name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case("ENO"))
            })
            .map(|arg| arg.value.clone());
        let enabled = self.condition(&en.value)?;
        // The value of a value-producing call: its type's declared default, unless the call runs.
        let result = match (&want, ret) {
            (Want::Nothing, _) => None,
            (Want::Scalar(_), Some(ret_id)) => {
                let Some(ret_ty) = crate::native::emit::return_type(self.model, Some(ret_id))?
                else {
                    return Err(self.unsupported(node, "EN on a call without a value"));
                };
                if !is_scalar(&self.info(ret_ty).kind) {
                    return Err(self.unsupported(node, "EN on a call whose value is no scalar"));
                }
                let temp = self.temp();
                let storage = self.info(ret_ty).rust.clone();
                self.line(&format!(
                    "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
                ));
                let place = Place {
                    expr: temp,
                    ty: ret_ty,
                    is_ref: false,
                };
                super::super::frame_init::start(self, &place, ret_id, None, "result")?;
                Some(place)
            }
            _ => return Err(self.unsupported(node, "EN on a call whose value is no scalar")),
        };
        self.line(&format!("if {enabled} {{"));
        self.indent += 1;
        let inner = match &result {
            Some(place) => Want::Scalar(place.ty),
            None => Want::Nothing,
        };
        let called = call(self, args, inner)?;
        if let (Some(place), Called::Scalar(value)) = (&result, &called) {
            self.store_scalar(place, value);
        }
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        if let Some(eno) = eno {
            self.store_false(&eno)?;
        }
        self.indent -= 1;
        self.line("}");
        match (result, want) {
            (Some(place), Want::Scalar(target)) => {
                let value = self.load(&place);
                Ok(Called::Scalar(
                    self.convert(node, &value, place.ty, target)?,
                ))
            }
            _ => Ok(Called::Nothing),
        }
    }

    /// Writes `FALSE` into the variable `target` names, an `ENO` of a disabled call.
    pub(super) fn store_false(&mut self, target: &SyntaxNode) -> Result<(), NativeError> {
        let destination = self.place(target)?;
        let bool_ty = self.model.types.elem(Elem::Bool);
        let value = self.convert(target, "false", bool_ty, destination.ty)?;
        self.store_scalar(&destination, &value);
        Ok(())
    }
}
