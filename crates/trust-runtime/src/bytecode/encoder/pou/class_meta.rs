impl<'a> BytecodeEncoder<'a> {
    fn class_meta<T>(
        &mut self,
        def: &T,
        explicit_owner: Option<&SmolStr>,
    ) -> Result<PouClassMeta, BytecodeError>
    where
        T: ClassLike,
    {
        let owner = explicit_owner.cloned().unwrap_or_else(|| def.name().clone());
        let parent_pou_id = def
            .base_name()
            .map(|base| {
                self.pou_ids
                    .class_like_id(&base)
                    .ok_or_else(|| BytecodeError::InvalidSection("unknown parent POU".into()))
            })
            .transpose()?;
        let methods = self.method_table_for(&owner)?;
        let interfaces = self.interface_impls_for(&owner, def.interfaces(), &methods)?;
        Ok(PouClassMeta {
            parent_pou_id,
            interfaces,
            methods,
        })
    }

    fn interface_impls_for(
        &mut self,
        owner: &SmolStr,
        declared: &[SmolStr],
        owner_methods: &[MethodEntry],
    ) -> Result<Vec<InterfaceImpl>, BytecodeError> {
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        for name in declared {
            self.collect_interface_and_bases(name, &mut seen, &mut names)?;
        }

        let mut mappings = Vec::with_capacity(names.len());
        for name in names {
            let type_id = self.runtime.registry().lookup(name.as_str()).ok_or_else(|| {
                BytecodeError::InvalidSection(
                    format!("unknown implemented interface '{name}' for '{owner}'").into(),
                )
            })?;
            let interface_type_id = self.type_index(type_id)?;
            let interface_methods = self.interface_methods_for(&name)?;
            let mut vtable_slots = Vec::with_capacity(interface_methods.len());
            for interface_method in interface_methods {
                let method_name = self
                    .strings
                    .entries
                    .get(interface_method.name_idx as usize)
                    .ok_or_else(|| BytecodeError::InvalidSection("interface method name missing".into()))?;
                let slot = owner_methods
                    .iter()
                    .find(|method| {
                        self.strings
                            .entries
                            .get(method.name_idx as usize)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(method_name))
                    })
                    .map(|method| method.vtable_slot)
                    .ok_or_else(|| {
                        BytecodeError::InvalidSection(
                            format!(
                                "implemented interface method '{method_name}' is missing from '{owner}'"
                            )
                            .into(),
                        )
                    })?;
                vtable_slots.push(slot);
            }
            mappings.push(InterfaceImpl {
                interface_type_id,
                vtable_slots,
            });
        }
        Ok(mappings)
    }

    fn collect_interface_and_bases(
        &self,
        name: &SmolStr,
        seen: &mut HashSet<SmolStr>,
        output: &mut Vec<SmolStr>,
    ) -> Result<(), BytecodeError> {
        let key = normalize_name(name);
        if !seen.insert(key.clone()) {
            return Ok(());
        }
        let interface = self.runtime.interfaces().get(&key).ok_or_else(|| {
            BytecodeError::InvalidSection(format!("unknown interface '{name}'").into())
        })?;
        output.push(interface.name.clone());
        if let Some(base) = &interface.base {
            self.collect_interface_and_bases(base, seen, output)?;
        }
        Ok(())
    }

    fn method_table_for(&mut self, owner: &SmolStr) -> Result<Vec<MethodEntry>, BytecodeError> {
        let key = normalize_name(owner);
        if let Some(existing) = self.method_tables.get(&key) {
            return Ok(existing.clone());
        }
        if self.method_stack.contains(&key) {
            return Err(BytecodeError::InvalidSection(
                "circular inheritance detected".into(),
            ));
        }
        self.method_stack.push(key.clone());

        let (base_name, methods) = match self.class_like_def(&key) {
            Some(def) => (def.base_name(), def.methods().to_vec()),
            None => return Err(BytecodeError::InvalidSection("unknown class-like".into())),
        };

        let mut table = Vec::new();
        let mut name_to_slot: HashMap<SmolStr, usize> = HashMap::new();

        if let Some(base) = base_name {
            let base_table = self.method_table_for(&base)?;
            for entry in &base_table {
                let method_name = self
                    .strings
                    .entries
                    .get(entry.name_idx as usize)
                    .cloned()
                    .unwrap_or_default();
                name_to_slot.insert(normalize_name(&method_name), entry.vtable_slot as usize);
                table.push(entry.clone());
            }
        }

        for method in &methods {
            let name = method.name.clone();
            let name_key = normalize_name(&name);
            let pou_id = self.method_id_for(owner, method)?;
            let name_idx = self.strings.intern(method.name.clone());
            if let Some(slot) = name_to_slot.get(&name_key).copied() {
                table[slot] = MethodEntry {
                    name_idx,
                    pou_id,
                    vtable_slot: slot as u32,
                    access: 0,
                    flags: 0,
                };
                continue;
            }
            let slot = table.len();
            name_to_slot.insert(name_key, slot);
            table.push(MethodEntry {
                name_idx,
                pou_id,
                vtable_slot: slot as u32,
                access: 0,
                flags: 0,
            });
        }

        self.method_stack.pop();
        self.method_tables.insert(key.clone(), table.clone());
        Ok(table)
    }

    fn method_id_for(&self, owner: &SmolStr, method: &MethodDef) -> Result<u32, BytecodeError> {
        self.pou_ids
            .method_id(owner, &method.name)
            .ok_or_else(|| BytecodeError::InvalidSection("method id missing".into()))
    }

    pub(super) fn interface_methods_for(
        &mut self,
        name: &SmolStr,
    ) -> Result<Vec<InterfaceMethod>, BytecodeError> {
        let key = normalize_name(name);
        if let Some(existing) = self.interface_tables.get(&key) {
            return Ok(existing.clone());
        }
        if self.interface_stack.contains(&key) {
            return Err(BytecodeError::InvalidSection(
                "circular interface inheritance detected".into(),
            ));
        }
        self.interface_stack.push(key.clone());

        let def = self
            .runtime
            .interfaces()
            .get(&key)
            .ok_or_else(|| BytecodeError::InvalidSection("unknown interface".into()))?;
        let base = def.base.clone();
        let methods = def.methods.clone();

        let mut table = Vec::new();
        let mut name_to_slot: HashMap<SmolStr, u32> = HashMap::new();

        if let Some(base_name) = base {
            let base_methods = self.interface_methods_for(&base_name)?;
            for method in &base_methods {
                let method_name = self
                    .strings
                    .entries
                    .get(method.name_idx as usize)
                    .cloned()
                    .unwrap_or_default();
                name_to_slot.insert(normalize_name(&method_name), method.slot);
                table.push(method.clone());
            }
        }

        for method in &methods {
            let name_idx = self.strings.intern(method.name.clone());
            let name_key = normalize_name(&method.name);
            if name_to_slot.contains_key(&name_key) {
                continue;
            }
            let slot = table.len() as u32;
            table.push(InterfaceMethod { name_idx, slot });
            name_to_slot.insert(name_key, slot);
        }

        self.interface_stack.pop();
        self.interface_tables.insert(key.clone(), table.clone());
        Ok(table)
    }

    fn class_like_def(&self, key: &SmolStr) -> Option<ClassLikeDef<'_>> {
        if let Some(fb) = self.runtime.function_blocks().get(key) {
            Some(ClassLikeDef::FunctionBlock(fb))
        } else {
            self.runtime.classes().get(key).map(ClassLikeDef::Class)
        }
    }

    fn self_fields_for_owner(
        &self,
        owner: &SmolStr,
    ) -> Result<HashMap<SmolStr, SmolStr>, BytecodeError> {
        let mut fields = HashMap::new();
        let mut seen = HashSet::new();
        let mut current = Some(owner.clone());
        while let Some(name) = current {
            let key = normalize_name(&name);
            if !seen.insert(key.clone()) {
                return Err(BytecodeError::InvalidSection(
                    "circular inheritance detected".into(),
                ));
            }
            let def = self
                .class_like_def(&key)
                .ok_or_else(|| BytecodeError::InvalidSection("unknown class-like".into()))?;
            match def {
                ClassLikeDef::FunctionBlock(fb) => {
                    for param in &fb.params {
                        insert_self_field(&mut fields, &param.name);
                    }
                    for var in &fb.vars {
                        insert_self_field(&mut fields, &var.name);
                    }
                    current = def.base_name();
                }
                ClassLikeDef::Class(class) => {
                    for var in &class.vars {
                        insert_self_field(&mut fields, &var.name);
                    }
                    current = class.base.clone();
                }
            }
        }
        Ok(fields)
    }
}
