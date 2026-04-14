pub(super) fn call_target_name(expr: &Expr) -> Option<SmolStr> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Field { target, field } => {
            let prefix = call_target_name(target)?;
            let mut combined = String::with_capacity(prefix.len() + field.len() + 1);
            combined.push_str(prefix.as_str());
            combined.push('.');
            combined.push_str(field.as_str());
            Some(combined.into())
        }
        _ => None,
    }
}

pub(crate) fn resolve_using_function<'a>(
    functions: &'a indexmap::IndexMap<SmolStr, crate::eval::FunctionDef>,
    name: &str,
    using: &[SmolStr],
) -> Option<&'a crate::eval::FunctionDef> {
    for namespace in using {
        let qualified = format!("{namespace}.{name}");
        let key = SmolStr::new(qualified.to_ascii_uppercase());
        if let Some(func) = functions.get(&key) {
            return Some(func);
        }
    }
    None
}

pub(crate) fn resolve_instance_method(
    ctx: &EvalContext<'_>,
    instance_id: InstanceId,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let instance = ctx.storage.get_instance(instance_id)?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());

    if let Some(function_blocks) = ctx.function_blocks {
        if let Some(fb) = function_blocks.get(&key) {
            let classes = ctx.classes?;
            return resolve_fb_method(function_blocks, classes, fb, name);
        }
    }

    let classes = ctx.classes?;
    let class_def = classes.get(&key)?;
    resolve_class_method(classes, class_def, name)
}

pub(super) fn resolve_fb_method(
    function_blocks: &indexmap::IndexMap<SmolStr, crate::eval::FunctionBlockDef>,
    classes: &indexmap::IndexMap<SmolStr, crate::eval::ClassDef>,
    fb: &crate::eval::FunctionBlockDef,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let mut current = Some(fb);
    while let Some(def) = current {
        if let Some(method) = def
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name))
        {
            return Some(method.clone());
        }
        let Some(base) = &def.base else {
            break;
        };
        match base {
            crate::program_model::FunctionBlockBase::FunctionBlock(base_name) => {
                let base_key = SmolStr::new(base_name.to_ascii_uppercase());
                current = function_blocks.get(&base_key);
            }
            crate::program_model::FunctionBlockBase::Class(base_name) => {
                let base_key = SmolStr::new(base_name.to_ascii_uppercase());
                let class_def = classes.get(&base_key)?;
                return resolve_class_method(classes, class_def, name);
            }
        }
    }
    None
}

pub(super) fn resolve_class_method(
    classes: &indexmap::IndexMap<SmolStr, crate::eval::ClassDef>,
    class_def: &crate::eval::ClassDef,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let mut current = class_def;
    loop {
        if let Some(method) = current
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name))
        {
            return Some(method.clone());
        }
        let Some(base) = &current.base else {
            break;
        };
        let base_key = SmolStr::new(base.to_ascii_uppercase());
        let Some(base_def) = classes.get(&base_key) else {
            break;
        };
        current = base_def;
    }
    None
}
