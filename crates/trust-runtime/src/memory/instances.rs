use super::access::ref_for_map;
use super::*;

impl VariableStorage {
    pub fn create_instance(&mut self, type_name: impl Into<SmolStr>) -> InstanceId {
        let id = InstanceId(self.next_instance_id);
        self.next_instance_id += 1;
        self.instances.insert(
            id,
            InstanceData {
                type_name: type_name.into(),
                variables: IndexMap::new(),
                parent: None,
            },
        );
        id
    }

    #[must_use]
    pub fn get_instance(&self, id: InstanceId) -> Option<&InstanceData> {
        self.instances.get(&id)
    }

    #[must_use]
    pub fn instances(&self) -> &FxHashMap<InstanceId, InstanceData> {
        &self.instances
    }

    pub fn get_instance_mut(&mut self, id: InstanceId) -> Option<&mut InstanceData> {
        self.instances.get_mut(&id)
    }

    pub fn set_instance_var(
        &mut self,
        id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        let name = name.into();
        let is_new = if let Some(instance) = self.instances.get_mut(&id) {
            let is_new = !instance.variables.contains_key(&name);
            instance.variables.insert(name, value);
            is_new
        } else {
            return false;
        };

        if is_new {
            self.invalidate_instance_field_caches(id);
        }
        true
    }

    #[must_use]
    pub fn get_instance_var(&self, id: InstanceId, name: &str) -> Option<&Value> {
        self.instances
            .get(&id)
            .and_then(|instance| instance.variables.get(name))
    }

    #[must_use]
    pub fn get_instance_var_recursive(&self, id: InstanceId, name: &str) -> Option<&Value> {
        let mut current = Some(id);
        while let Some(instance_id) = current {
            if let Some(value) = self.get_instance_var(instance_id, name) {
                return Some(value);
            }
            current = self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
        }
        None
    }

    pub fn ref_for_global(&self, name: &str) -> Option<crate::value::ValueRef> {
        ref_for_map(&self.globals, MemoryLocation::Global, name)
    }

    pub fn ref_for_local(&self, name: &str) -> Option<crate::value::ValueRef> {
        let frame = self.current_frame()?;
        ref_for_map(&frame.variables, MemoryLocation::Local(frame.id), name)
    }

    pub fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<crate::value::ValueRef> {
        let field_name = SmolStr::new(name);
        let offset = self.cached_instance_field_offset(id, &field_name)?;
        Some(crate::value::ValueRef {
            location: MemoryLocation::Instance(id),
            offset,
            path: RefPath::new(),
        })
    }

    pub fn ref_for_instance_recursive(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let field_name = SmolStr::new(name);
        if let Some(resolution) = self.cached_recursive_instance_field_resolution(id, &field_name) {
            let owner = self.resolve_ancestor_instance(id, resolution.owner_depth)?;
            return Some(crate::value::ValueRef {
                location: MemoryLocation::Instance(owner),
                offset: resolution.offset,
                path: RefPath::new(),
            });
        }

        let mut current = Some(id);
        let mut owner_depth = 0usize;
        while let Some(instance_id) = current {
            if let Some(offset) = self.cached_instance_field_offset(instance_id, &field_name) {
                let resolution = RecursiveInstanceFieldResolution {
                    owner_depth,
                    offset,
                };
                self.cache_recursive_instance_field_resolution(id, &field_name, resolution);
                return Some(crate::value::ValueRef {
                    location: MemoryLocation::Instance(instance_id),
                    offset,
                    path: RefPath::new(),
                });
            }
            current = self
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
            owner_depth += 1;
        }
        None
    }

    fn invalidate_instance_field_caches(&self, id: InstanceId) {
        self.instance_field_offsets
            .write()
            .expect("cache poisoned")
            .retain(|(instance_id, _), _| *instance_id != id);
        self.recursive_instance_field_resolutions
            .write()
            .expect("cache poisoned")
            .retain(|(instance_id, _), _| *instance_id != id);
    }

    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &SmolStr) -> Option<usize> {
        let key = (id, field_name.clone());
        if let Some(cached) = self
            .instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&key)
            .copied()
        {
            return cached;
        }

        let offset = self
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index_of(field_name.as_str()));
        self.instance_field_offsets
            .write()
            .expect("cache poisoned")
            .insert(key, offset);
        offset
    }

    fn cached_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &SmolStr,
    ) -> Option<RecursiveInstanceFieldResolution> {
        self.recursive_instance_field_resolutions
            .read()
            .expect("cache poisoned")
            .get(&(id, field_name.clone()))
            .copied()
    }

    fn cache_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &SmolStr,
        resolution: RecursiveInstanceFieldResolution,
    ) {
        self.recursive_instance_field_resolutions
            .write()
            .expect("cache poisoned")
            .insert((id, field_name.clone()), resolution);
    }

    fn resolve_ancestor_instance(&self, id: InstanceId, depth: usize) -> Option<InstanceId> {
        let mut current = id;
        for _ in 0..depth {
            current = self.instances.get(&current)?.parent?;
        }
        Some(current)
    }

    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        let instance = self.instances.get(&id)?;
        let field_name = SmolStr::new(name);
        let key = (instance.type_name.clone(), field_name.clone());
        if let Some(offset) = self
            .declared_instance_field_offsets
            .read()
            .expect("cache poisoned")
            .get(&key)
            .copied()
        {
            return Some(offset);
        }

        let offset = instance.variables.get_index_of(field_name.as_str())?;
        self.declared_instance_field_offsets
            .write()
            .expect("cache poisoned")
            .insert(key, offset);
        Some(offset)
    }

    pub fn declared_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let offset = self.declared_instance_field_offset(id, name)?;
        Some(crate::value::ValueRef {
            location: MemoryLocation::Instance(id),
            offset,
            path: RefPath::new(),
        })
    }

    pub fn resolved_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        self.declared_instance_field_ref(id, name)
            .or_else(|| self.ref_for_instance_recursive(id, name))
    }

    pub fn read_instance_field_by_offset(&self, id: InstanceId, offset: usize) -> Option<&Value> {
        self.instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value))
    }

    pub fn write_instance_field_by_offset(
        &mut self,
        id: InstanceId,
        offset: usize,
        value: Value,
    ) -> bool {
        self.instances
            .get_mut(&id)
            .and_then(|instance| {
                instance.variables.get_index_mut(offset).map(|(_, slot)| {
                    *slot = value;
                })
            })
            .is_some()
    }
}
