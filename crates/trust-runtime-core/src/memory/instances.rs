use super::access::ref_for_map;
use super::*;

impl VariableStorage {
    /// Allocate an instance identity without wrapping the live reference namespace.
    pub fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, crate::error::RuntimeError> {
        let next = self
            .next_instance_id
            .checked_add(1)
            .ok_or(crate::error::RuntimeError::Overflow)?;
        let id = InstanceId(self.next_instance_id);
        let value = InstanceData {
            type_name: type_name.into(),
            variables: IndexMap::default(),
            parent: None,
        };
        #[cfg(feature = "std")]
        self.instances.insert(id, value);
        #[cfg(not(feature = "std"))]
        self.instances.try_append(id, value)?;
        self.next_instance_id = next;
        Ok(id)
    }

    /// Array growth bytes and moved entries to charge before creating an instance.
    /// Hosted map nodes retain the existing per-instance construction accounting.
    pub fn instance_insertion_demand(&self) -> Option<(usize, usize)> {
        #[cfg(feature = "std")]
        {
            Some((0, 0))
        }
        #[cfg(not(feature = "std"))]
        {
            if self.instances.len() < self.instances.capacity() {
                return Some((0, 0));
            }
            let bytes = self
                .instances
                .growth_capacity()?
                .checked_mul(core::mem::size_of::<(InstanceId, InstanceData)>())?;
            Some((bytes, self.instances.len()))
        }
    }

    /// Release an invocation-owned instance and invalidate its lookup caches.
    pub fn remove_instance(&mut self, id: InstanceId) -> Option<InstanceData> {
        self.invalidate_instance_field_caches(id);
        self.instances.remove(&id)
    }

    /// Retire an owner batch in one pass. Callers charge the live entry count first.
    /// Cleanup itself cannot fail or consume additional execution fuel.
    pub fn retain_instances(&mut self, mut keep: impl FnMut(InstanceId) -> bool) -> usize {
        let before = self.instances.len();
        self.instances.retain(|id, _| keep(*id));
        let removed = before - self.instances.len();
        #[cfg(feature = "std")]
        if removed != 0 {
            cache::exclusive(&mut self.instance_field_offsets).clear();
            cache::exclusive(&mut self.recursive_instance_field_resolutions).clear();
        }
        removed
    }

    /// Legacy infallible host construction; admitted execution uses try_create_instance.
    pub fn create_instance(&mut self, type_name: impl Into<SmolStr>) -> InstanceId {
        self.try_create_instance(type_name)
            .expect("instance identity or allocation exhausted")
    }

    #[must_use]
    pub fn get_instance(&self, id: InstanceId) -> Option<&InstanceData> {
        self.instances.get(&id)
    }

    #[must_use]
    pub fn instances(&self) -> &InstanceMap {
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
        let offset = self.cached_instance_field_offset(id, name)?;
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
        let field_name = name;
        if let Some(resolution) = self.cached_recursive_instance_field_resolution(id, field_name) {
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
            if let Some(offset) = self.cached_instance_field_offset(instance_id, field_name) {
                let resolution = RecursiveInstanceFieldResolution {
                    owner_depth,
                    offset,
                };
                self.cache_recursive_instance_field_resolution(id, field_name, resolution);
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

    #[cfg(not(feature = "std"))]
    fn invalidate_instance_field_caches(&mut self, _: InstanceId) {}

    #[cfg(feature = "std")]
    fn invalidate_instance_field_caches(&mut self, id: InstanceId) {
        cache::exclusive(&mut self.instance_field_offsets).retain(|(owner, _), _| *owner != id);
        cache::exclusive(&mut self.recursive_instance_field_resolutions)
            .retain(|(owner, _), _| *owner != id);
    }

    #[cfg(not(feature = "std"))]
    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &str) -> Option<usize> {
        self.instances.get(&id)?.variables.get_index_of(field_name)
    }

    #[cfg(feature = "std")]
    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &str) -> Option<usize> {
        let key = (id, SmolStr::new(field_name));
        if let Some(cached) = recover_read_lock(self.instance_field_offsets.read())
            .and_then(|cache| cache.get(&key).copied())
        {
            return cached;
        }

        let offset = self
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index_of(field_name));
        if let Ok(mut cache) = self.instance_field_offsets.write() {
            cache.insert(key, offset);
        }
        offset
    }

    #[cfg(feature = "std")]
    fn cached_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &str,
    ) -> Option<RecursiveInstanceFieldResolution> {
        recover_read_lock(self.recursive_instance_field_resolutions.read())
            .and_then(|cache| cache.get(&(id, SmolStr::new(field_name))).copied())
    }

    #[cfg(feature = "std")]
    fn cache_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &str,
        resolution: RecursiveInstanceFieldResolution,
    ) {
        if let Ok(mut cache) = self.recursive_instance_field_resolutions.write() {
            cache.insert((id, SmolStr::new(field_name)), resolution);
        }
    }

    #[cfg(not(feature = "std"))]
    fn cached_recursive_instance_field_resolution(
        &self,
        _: InstanceId,
        _: &str,
    ) -> Option<RecursiveInstanceFieldResolution> {
        None
    }

    #[cfg(not(feature = "std"))]
    fn cache_recursive_instance_field_resolution(
        &self,
        _: InstanceId,
        _: &str,
        _: RecursiveInstanceFieldResolution,
    ) {
    }

    fn resolve_ancestor_instance(&self, id: InstanceId, depth: usize) -> Option<InstanceId> {
        let mut current = id;
        for _ in 0..depth {
            current = self.instances.get(&current)?.parent?;
        }
        Some(current)
    }

    #[cfg(not(feature = "std"))]
    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        self.instances.get(&id)?.variables.get_index_of(name)
    }

    #[cfg(feature = "std")]
    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        let instance = self.instances.get(&id)?;
        let field_name = SmolStr::new(name);
        let key = (instance.type_name.clone(), field_name.clone());
        if let Some(offset) = recover_read_lock(self.declared_instance_field_offsets.read())
            .and_then(|cache| cache.get(&key).copied())
        {
            return Some(offset);
        }

        let offset = instance.variables.get_index_of(field_name.as_str())?;
        if let Ok(mut cache) = self.declared_instance_field_offsets.write() {
            cache.insert(key, offset);
        }
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
