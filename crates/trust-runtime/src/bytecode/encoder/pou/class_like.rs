trait ClassLike {
    fn name(&self) -> &SmolStr;
    fn base_name(&self) -> Option<SmolStr>;
    fn methods(&self) -> &[MethodDef];
}

impl ClassLike for FunctionBlockDef {
    fn name(&self) -> &SmolStr {
        &self.name
    }

    fn base_name(&self) -> Option<SmolStr> {
        self.base.as_ref().map(|base| match base {
            crate::program_model::FunctionBlockBase::FunctionBlock(name)
            | crate::program_model::FunctionBlockBase::Class(name) => name.clone(),
        })
    }

    fn methods(&self) -> &[MethodDef] {
        &self.methods
    }
}

impl ClassLike for ClassDef {
    fn name(&self) -> &SmolStr {
        &self.name
    }

    fn base_name(&self) -> Option<SmolStr> {
        self.base.clone()
    }

    fn methods(&self) -> &[MethodDef] {
        &self.methods
    }
}

enum ClassLikeDef<'a> {
    FunctionBlock(&'a FunctionBlockDef),
    Class(&'a ClassDef),
}

impl<'a> ClassLikeDef<'a> {
    fn base_name(&self) -> Option<SmolStr> {
        match self {
            ClassLikeDef::FunctionBlock(def) => def.base_name(),
            ClassLikeDef::Class(def) => def.base_name(),
        }
    }

    fn methods(&self) -> &[MethodDef] {
        match self {
            ClassLikeDef::FunctionBlock(def) => def.methods(),
            ClassLikeDef::Class(def) => def.methods(),
        }
    }
}

fn insert_self_field(map: &mut HashMap<SmolStr, SmolStr>, name: &SmolStr) {
    let key = normalize_name(name);
    map.entry(key).or_insert_with(|| name.clone());
}
