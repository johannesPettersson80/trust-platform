//! Instances as objects (IEC §6.6.5 to §6.6.8; specification 44 §6.4): `THIS` and `SUPER`, method
//! calls bound statically or by the instance's class, calls through interface references, the
//! values of interfaces, and properties as calls of their `GET` and `SET`.

use smol_str::SmolStr;
use trust_hir::SymbolKind;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::call::{ident, Arg, Called, Want};
use super::{Frame, Lowerer, Need, Place};
use crate::native::dispatch::{interface_function, virtual_function};
use crate::native::emit::method_function;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;
use crate::program_model::property_setter_method_name;

/// What a method call is called on.
pub(super) enum Receiver {
    /// An instance; `exact` for `SUPER`, whose method is the base's own, never an override.
    Instance { place: Place, exact: bool },
    /// The instance an interface reference names: the reference's value, bound once, and the
    /// interface's name.
    Interface {
        reference: String,
        interface: SmolStr,
    },
}

impl<'a> Lowerer<'_, 'a> {
    /// `THIS`: the instance of the function block or class whose body or method this is.
    pub(super) fn this_place(&self, node: &SyntaxNode) -> Result<Place, NativeError> {
        match self.frame {
            Frame::FunctionBlock(ty) | Frame::Method(ty) => Ok(Place {
                expr: "me".into(),
                ty,
                is_ref: true,
            }),
            _ => Err(self.unsupported(node, "THIS outside a function block or a class")),
        }
    }

    /// `SUPER`: the instance's part of its base's type, which lies first (§6.4).
    fn super_place(&self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let this = self.this_place(node)?;
        let Kind::Fb {
            base: Some(base), ..
        } = self.info(this.ty).kind
        else {
            return Err(self.unsupported(node, "SUPER of a type that extends none"));
        };
        Ok(Place {
            expr: "me.__BASE".into(),
            ty: base,
            is_ref: false,
        })
    }

    /// The place `node` names when it is `THIS`, `THIS^` or `SUPER`.
    pub(super) fn object_place(&self, node: &SyntaxNode) -> Option<Result<Place, NativeError>> {
        match node.kind() {
            SyntaxKind::ThisExpr => Some(self.this_place(node)),
            SyntaxKind::SuperExpr => Some(self.super_place(node)),
            SyntaxKind::DerefExpr
                if node
                    .children()
                    .next()
                    .is_some_and(|inner| inner.kind() == SyntaxKind::ThisExpr) =>
            {
                Some(self.this_place(node))
            }
            _ => None,
        }
    }

    /// What the method call on `base` is called on.
    pub(super) fn receiver(&mut self, base: &SyntaxNode) -> Result<Receiver, NativeError> {
        if base.kind() == SyntaxKind::SuperExpr {
            return Ok(Receiver::Instance {
                place: self.super_place(base)?,
                exact: true,
            });
        }
        if let Some(place) = self.object_place(base) {
            return Ok(Receiver::Instance {
                place: place?,
                exact: false,
            });
        }
        let ty = self.type_of(base)?;
        if matches!(self.info(ty).kind, Kind::Interface) {
            let interface = self.info(ty).name.clone();
            let value = self.interface_value(base)?;
            let reference = self.bind(&value);
            return Ok(Receiver::Interface {
                reference,
                interface,
            });
        }
        let place = self.place(base)?;
        if !matches!(self.info(place.ty).kind, Kind::Fb { .. }) {
            return Err(self.unsupported(base, "a method of a value that is no function block"));
        }
        Ok(Receiver::Instance {
            place,
            exact: false,
        })
    }

    /// The reference to `place` as a variable of type `ty`: itself, or its part of a base type
    /// that lies first in it (IEC §6.6.8.3).
    pub(super) fn as_type(
        &self,
        node: &SyntaxNode,
        place: &Place,
        ty: Ty,
    ) -> Result<String, NativeError> {
        if place.ty == ty {
            return Ok(place.reference());
        }
        match self.model.types.upcast(place.ty, ty) {
            Some(path) => Ok(format!("&{}{path}", place.expr)),
            None => Err(self.unsupported(node, "a variable of another type")),
        }
    }

    /// The call of method `name` on `receiver` with `args`.
    pub(super) fn member_call(
        &mut self,
        node: &SyntaxNode,
        receiver: Receiver,
        name: &str,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        let defs = self.model.types.defs;
        match receiver {
            Receiver::Instance { place, exact } => {
                let Some(owner) = self.model.types.owner_of(place.ty) else {
                    return Err(
                        self.unsupported(node, "a method of a value that is no function block")
                    );
                };
                let Some((declaring, method)) = defs.method(owner, name) else {
                    return Err(self.unsupported(node, format!("the method {name}")));
                };
                let bound = self.bind_arguments(node, &method.params, args)?;
                let (arguments, outputs) = self.inputs(node, &bound)?;
                let (function, receiver) = if !exact && defs.overridden_below(owner, name) {
                    // A derived type overrides it: the instance's class decides (IEC §6.6.8).
                    Need::Virtual(place.ty, method.name.clone()).add_to(&mut self.needs);
                    (
                        virtual_function(self.model, place.ty, &method.name),
                        place.reference(),
                    )
                } else {
                    let Some(declaring_ty) = self.model.types.by_rust(&declaring.rust()) else {
                        return Err(
                            self.unsupported(node, format!("the layout of {}", declaring.name()))
                        );
                    };
                    let receiver = self.as_type(node, &place, declaring_ty)?;
                    Need::Method(declaring.name().clone(), method.name.clone())
                        .add_to(&mut self.needs);
                    (method_function(declaring, method), receiver)
                };
                let called = self.emit_call(
                    node,
                    &function,
                    Some(receiver),
                    arguments,
                    method.return_type,
                    &want,
                )?;
                self.copy_outputs(outputs)?;
                Ok(called)
            }
            Receiver::Interface {
                reference,
                interface,
            } => {
                let Some(method) = defs.interface_method(&interface, name) else {
                    return Err(self.unsupported(node, format!("the method {interface}.{name}")));
                };
                let bound = self.bind_arguments(node, &method.params, args)?;
                let (arguments, outputs) = self.inputs(node, &bound)?;
                Need::Interface(interface.clone(), method.name.clone()).add_to(&mut self.needs);
                let function = interface_function(&interface, &method.name);
                let called = self.emit_call(
                    node,
                    &function,
                    Some(format!("&{reference}")),
                    arguments,
                    method.return_type,
                    &want,
                )?;
                self.copy_outputs(outputs)?;
                Ok(called)
            }
        }
    }

    /// The value of an interface (IEC §6.6.6): `NULL`, an instance, `THIS`, another interface's
    /// value, or the result of a call.
    pub(super) fn interface_value(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.interface_value(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal if super::expr::is_null(node) => {
                Ok("abi::RefValue::NULL_REF".into())
            }
            SyntaxKind::CallExpr => {
                let ty = self.type_of(node)?;
                let place = self.call_aggregate(node, ty)?;
                Ok(format!("{}.get()", place.expr))
            }
            _ => {
                let place = match self.object_place(node) {
                    Some(place) => place?,
                    None => self.place(node)?,
                };
                match self.info(place.ty).kind {
                    Kind::Interface => Ok(format!("{}.get()", place.expr)),
                    Kind::Fb { .. } => Ok(self.bind(&format!(
                        "w.areas.refer(cx, {}, abi::RefValue::INSTANCE)?",
                        place.reference()
                    ))),
                    _ => {
                        Err(self.unsupported(node, "a value that is no instance for an interface"))
                    }
                }
            }
        }
    }

    /// The property `node` names, by its name: a member of an instance or of an interface, or,
    /// inside a method, of the instance itself; a frame variable of the name is the variable.
    pub(super) fn property_name(&self, node: &SyntaxNode) -> Option<String> {
        let name = match node.kind() {
            SyntaxKind::NameRef => {
                let name = ident(node);
                if self
                    .locals
                    .iter()
                    .any(|local| local.name.eq_ignore_ascii_case(&name))
                {
                    return None;
                }
                name
            }
            SyntaxKind::FieldExpr => ident(&node.children().nth(1)?),
            _ => return None,
        };
        let symbol = self.src.symbol_of(node)?;
        let symbol = self.src.analysis.symbols.get(symbol)?;
        matches!(symbol.kind, SymbolKind::Property { .. }).then_some(name)
    }

    /// What a property of `node` is read or written on: its instance or interface, or the
    /// instance itself.
    fn property_receiver(&mut self, node: &SyntaxNode) -> Result<Receiver, NativeError> {
        match node.kind() {
            SyntaxKind::FieldExpr => {
                let Some(base) = node.children().next() else {
                    return Err(self.unsupported(node, "a property without its instance"));
                };
                self.receiver(&base)
            }
            _ => Ok(Receiver::Instance {
                place: self.this_place(node)?,
                exact: false,
            }),
        }
    }

    /// The value of the property `node` reads, from a call of its `GET`, in a temporary of its
    /// type; `None` when `node` names no property.
    pub(super) fn property_read(
        &mut self,
        node: &SyntaxNode,
    ) -> Result<Option<Place>, NativeError> {
        let Some(name) = self.property_name(node) else {
            return Ok(None);
        };
        let receiver = self.property_receiver(node)?;
        let ty = self.type_of(node)?;
        match self.member_call(node, receiver, &name, Vec::new(), Want::Aggregate)? {
            Called::Scalar(value) => {
                let storage = self.info(ty).rust.clone();
                let stored = match self.info(ty).scalar() {
                    Some(Elem::Bool) => format!("u8::from({value})"),
                    _ => value,
                };
                let temp = self.bind(&format!("<{storage}>::new({stored})"));
                Ok(Some(Place {
                    expr: temp,
                    ty,
                    is_ref: false,
                }))
            }
            Called::Place(place) => Ok(Some(place)),
            _ => Err(self.unsupported(node, format!("the property {name} without a value"))),
        }
    }

    /// `target := value` for a property `target`: a call of its `SET` with the value; whether
    /// `target` is a property.
    pub(super) fn property_write(
        &mut self,
        target: &SyntaxNode,
        value: &SyntaxNode,
    ) -> Result<bool, NativeError> {
        let Some(name) = self.property_name(target) else {
            return Ok(false);
        };
        let receiver = self.property_receiver(target)?;
        let setter = property_setter_method_name(&SmolStr::new(&name));
        let argument = Arg {
            name: None,
            output: false,
            value: value.clone(),
        };
        self.member_call(target, receiver, &setter, vec![argument], Want::Nothing)?;
        Ok(true)
    }
}
