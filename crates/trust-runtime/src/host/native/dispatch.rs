//! The dispatchers of a compiled program (IEC §6.6.8; specification 44 §6.4): a call bound by the
//! class of the instance it reaches, through an interface reference, or on a variable, a
//! `VAR_IN_OUT`, a reference or `THIS` whose type has derived types that override the method or
//! have a body of their own.
//!
//! A dispatcher is a `match` on the instance's class id over every type of the program that can be
//! there, each arm a direct call of the method that class reaches; the build knows every type, so
//! there is no table of methods in the program. A type that is never laid out has no instance and
//! no arm.

use std::fmt::Write as _;

use smol_str::SmolStr;

use super::classes::Owner;
use super::emit::{method_function, signature, Signature};
use super::lower::Need;
use super::model::Model;
use super::recursion::CallGraph;
use super::types::{mangle, Kind, Ty};
use super::NativeError;
use crate::program_model::MethodDef;

/// One arm of a dispatcher: the class id, the type of its instances, and the method that class
/// reaches with the type that declares it.
struct Arm<'a> {
    class: u32,
    ty: Ty,
    owner: Owner<'a>,
    method: &'a MethodDef,
}

/// The name of the dispatcher of method `method` on instances of type `ty` and its derived types.
pub(super) fn virtual_function(model: &Model<'_>, ty: Ty, method: &str) -> String {
    format!(
        "v_{}__{}",
        mangle(&model.types.info(ty).name),
        mangle(method)
    )
}

/// The name of the dispatcher of method `method` through interface `interface`.
pub(super) fn interface_function(interface: &str, method: &str) -> String {
    format!("i_{}__{}", mangle(interface), mangle(method))
}

/// The name of the dispatcher of the body of instances of type `ty` and its derived types.
pub(super) fn body_function(model: &Model<'_>, ty: Ty) -> String {
    format!("b_{}", mangle(&model.types.info(ty).name))
}

/// The method a call on an instance of type `ty` reaches, with the type that declares it.
fn reached<'a>(
    model: &Model<'a>,
    ty: Ty,
    name: &str,
) -> Result<(Owner<'a>, &'a MethodDef), NativeError> {
    let owner = model.types.owner_of(ty).ok_or_else(|| {
        NativeError::Compile(format!(
            "{} is no function block or class",
            model.types.info(ty).name
        ))
    })?;
    model.types.defs.method(owner, name).ok_or_else(|| {
        NativeError::Compile(format!("method {}.{name} is not declared", owner.name()))
    })
}

/// The arms of a call of method `name` on an instance of type `ty` or of a derived type: each
/// derived type that reaches another method than `ty` reaches.
fn virtual_arms<'a>(
    model: &Model<'a>,
    ty: Ty,
    name: &str,
) -> Result<(Owner<'a>, &'a MethodDef, Vec<Arm<'a>>), NativeError> {
    let (owner, method) = reached(model, ty, name)?;
    let mut arms = Vec::new();
    for (class_ty, class) in model.types.classes() {
        if class_ty == ty || model.types.upcast(class_ty, ty).is_none() {
            continue;
        }
        let (class_owner, class_method) = reached(model, class_ty, name)?;
        if !class_owner.same(owner) {
            arms.push(Arm {
                class,
                ty: class_ty,
                owner: class_owner,
                method: class_method,
            });
        }
    }
    Ok((owner, method, arms))
}

/// The arms of a call of method `name` through interface `interface`: each type that implements
/// it. A type without the method, an abstract base, has no instance of its own and no arm.
fn interface_arms<'a>(
    model: &Model<'a>,
    interface: &str,
    name: &str,
) -> Result<Vec<Arm<'a>>, NativeError> {
    let defs = model.types.defs;
    let mut arms = Vec::new();
    for (class_ty, class) in model.types.classes() {
        let Some(class_owner) = model.types.owner_of(class_ty) else {
            continue;
        };
        if !defs.implements(class_owner, interface) {
            continue;
        }
        if let Some((owner, method)) = defs.method(class_owner, name) {
            arms.push(Arm {
                class,
                ty: class_ty,
                owner,
                method,
            });
        }
    }
    Ok(arms)
}

/// The methods the dispatchers `needs` call, which the crate must hold.
pub(super) fn targets(model: &Model<'_>, needs: &[Need]) -> Result<Vec<Need>, NativeError> {
    let mut targets = Vec::new();
    for need in needs {
        let arms = match need {
            Need::Virtual(ty, name) => {
                let (owner, method, arms) = virtual_arms(model, *ty, name)?;
                Need::Method(owner.name().clone(), method.name.clone()).add_to(&mut targets);
                arms
            }
            Need::Interface(interface, name) => interface_arms(model, interface, name)?,
            _ => Vec::new(),
        };
        for arm in arms {
            Need::Method(arm.owner.name().clone(), arm.method.name.clone()).add_to(&mut targets);
        }
    }
    Ok(targets)
}

/// The call of `owner`'s `method` on the instance `instance` of type `ty`, a reference, passing on
/// `names`.
fn arm_call(
    model: &Model<'_>,
    instance: &str,
    ty: Ty,
    owner: Owner<'_>,
    method: &MethodDef,
    names: &[String],
) -> Result<String, NativeError> {
    let Some(owner_ty) = model.types.by_rust(&owner.rust()) else {
        return Err(NativeError::Compile(format!(
            "function block {} has no layout",
            owner.name()
        )));
    };
    let path = model.types.upcast(ty, owner_ty).ok_or_else(|| {
        NativeError::Compile(format!(
            "{} is no base of {}",
            owner.name(),
            model.types.info(ty).name
        ))
    })?;
    let receiver = if path.is_empty() {
        instance.to_string()
    } else {
        format!("&{instance}{path}")
    };
    let mut arguments = vec!["cx".to_string(), "w".to_string(), receiver];
    arguments.extend(names.iter().cloned());
    Ok(format!(
        "{}({})",
        method_function(owner, method),
        arguments.join(", ")
    ))
}

/// The statements that view the instance `me` of type `from` as the instance of class `class`, type
/// `to`, named `s`: a reference taken as `from`, then the instance its class id says lies there.
fn downcast(model: &Model<'_>, to: Ty, class: u32) -> String {
    format!(
        "let r = w.areas.refer(cx, me, abi::RefValue::INSTANCE)?;\n            let s: &{} = w.areas.instance(cx, &r, {class})?;",
        model.types.info(to).rust
    )
}

/// Checks that `implementation`, the method a class reaches, takes and returns what `declared`,
/// the method a dispatcher is called as, does.
fn same_signature(
    model: &mut Model<'_>,
    declared: (&SmolStr, &MethodDef),
    implementation: (Owner<'_>, &MethodDef),
) -> Result<(), NativeError> {
    let (owner, method) = implementation;
    let mismatch = || {
        NativeError::Compile(format!(
            "{}.{} does not take and return what {}.{} does",
            owner.name(),
            method.name,
            declared.0,
            declared.1.name
        ))
    };
    let (declared_ret, implemented_ret) = (
        super::emit::return_type(model, declared.1.return_type)?,
        super::emit::return_type(model, method.return_type)?,
    );
    if declared_ret != implemented_ret || declared.1.params.len() != method.params.len() {
        return Err(mismatch());
    }
    for param in &method.params {
        let Some(wanted) = declared
            .1
            .params
            .iter()
            .find(|wanted| wanted.name.eq_ignore_ascii_case(&param.name))
        else {
            return Err(mismatch());
        };
        if wanted.direction != param.direction
            || model.ty(wanted.type_id)? != model.ty(param.type_id)?
        {
            return Err(mismatch());
        }
    }
    Ok(())
}

/// The dispatchers `needs` names, as Rust functions.
pub(super) fn emit(
    model: &mut Model<'_>,
    needs: &[Need],
    graph: &mut CallGraph,
) -> Result<String, NativeError> {
    let mut out = String::new();
    for need in needs {
        match need {
            Need::Virtual(ty, name) => {
                let (owner, method, arms) = virtual_arms(model, *ty, name)?;
                let Signature {
                    parameters,
                    names,
                    returns,
                } = signature(model, method.return_type, &method.params)?;
                let dispatcher = virtual_function(model, *ty, name);
                graph.dispatcher(&dispatcher, &method_function(owner, method));
                for arm in &arms {
                    graph.dispatcher(&dispatcher, &method_function(arm.owner, arm.method));
                }
                let _ = writeln!(
                    out,
                    "fn {dispatcher}(cx: &mut Cx, w: &World<'_>, me: &{}{parameters}) -> R<{returns}> {{",
                    model.types.info(*ty).rust
                );
                let field = class_field(model, *ty)?;
                let _ = writeln!(out, "    match me{field}.get() {{");
                for arm in &arms {
                    same_signature(model, (owner.name(), method), (arm.owner, arm.method))?;
                    let call = arm_call(model, "s", arm.ty, arm.owner, arm.method, &names)?;
                    let _ = writeln!(
                        out,
                        "        {} => {{\n            {}\n            {call}\n        }}",
                        arm.class,
                        downcast(model, arm.ty, arm.class)
                    );
                }
                let call = arm_call(model, "me", *ty, owner, method, &names)?;
                let _ = writeln!(out, "        _ => {call},\n    }}\n}}\n");
            }
            Need::Interface(interface, name) => {
                let defs = model.types.defs;
                let Some(declared) = defs.interface_method(interface, name) else {
                    return Err(NativeError::Compile(format!(
                        "method {interface}.{name} is not declared"
                    )));
                };
                let interface_name = defs
                    .interface(interface)
                    .map_or_else(|| interface.clone(), |def| def.name.clone());
                let arms = interface_arms(model, interface, name)?;
                let Signature {
                    parameters,
                    names,
                    returns,
                } = signature(model, declared.return_type, &declared.params)?;
                let dispatcher = interface_function(interface, name);
                for arm in &arms {
                    graph.dispatcher(&dispatcher, &method_function(arm.owner, arm.method));
                }
                let _ = writeln!(
                    out,
                    "fn {dispatcher}(cx: &mut Cx, w: &World<'_>, r: &abi::RefValue{parameters}) -> R<{returns}> {{\n    match w.areas.class_of(cx, r)? {{"
                );
                for arm in &arms {
                    same_signature(model, (&interface_name, declared), (arm.owner, arm.method))?;
                    let call = arm_call(model, "s", arm.ty, arm.owner, arm.method, &names)?;
                    let _ = writeln!(
                        out,
                        "        {} => {{\n            let s: &{} = w.areas.instance(cx, r, {})?;\n            {call}\n        }}",
                        arm.class,
                        model.types.info(arm.ty).rust,
                        arm.class
                    );
                }
                let _ = writeln!(
                    out,
                    "        other => Err(fault(cx, abi::FaultCode::TypeMismatch, i64::from(other), 0, 0)),\n    }}\n}}\n"
                );
            }
            Need::Body(ty) => {
                let dispatcher = body_function(model, *ty);
                let _ = writeln!(
                    out,
                    "fn {dispatcher}(cx: &mut Cx, w: &World<'_>, me: &{}) -> R<()> {{",
                    model.types.info(*ty).rust
                );
                let field = class_field(model, *ty)?;
                let _ = writeln!(out, "    match me{field}.get() {{");
                for (class_ty, class) in model.types.classes() {
                    let derived = class_ty != *ty
                        && model.types.upcast(class_ty, *ty).is_some()
                        && matches!(
                            model.types.info(class_ty).kind,
                            Kind::Fb {
                                is_class: false,
                                ..
                            }
                        );
                    if !derived {
                        continue;
                    }
                    let derived_body = format!("fb_{}", mangle(&model.types.info(class_ty).name));
                    graph.dispatcher(&dispatcher, &derived_body);
                    let _ = writeln!(
                        out,
                        "        {class} => {{\n            {}\n            {derived_body}(cx, w, s)\n        }}",
                        downcast(model, class_ty, class)
                    );
                }
                let own_body = format!("fb_{}", mangle(&model.types.info(*ty).name));
                graph.dispatcher(&dispatcher, &own_body);
                let _ = writeln!(out, "        _ => {own_body}(cx, w, me),\n    }}\n}}\n");
            }
            Need::Function(_) | Need::Method(..) => {}
        }
    }
    Ok(out)
}

/// The path from an instance of type `ty` to its class id.
fn class_field(model: &Model<'_>, ty: Ty) -> Result<String, NativeError> {
    model.types.class_field(ty).ok_or_else(|| {
        NativeError::Compile(format!("{} holds no class id", model.types.info(ty).name))
    })
}
