use super::*;

#[derive(Debug, Clone)]
pub struct AccessBinding {
    pub name: SmolStr,
    pub reference: ValueRef,
    pub partial: Option<PartialAccess>,
}

#[derive(Debug, Default, Clone)]
pub struct AccessMap {
    bindings: IndexMap<SmolStr, AccessBinding>,
}

impl AccessMap {
    pub fn bind(&mut self, name: SmolStr, reference: ValueRef, partial: Option<PartialAccess>) {
        let binding = AccessBinding {
            name: name.clone(),
            reference,
            partial,
        };
        self.bindings.insert(name, binding);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AccessBinding> {
        self.bindings.get(name)
    }
}

pub(super) fn ref_for_map(
    map: &IndexMap<SmolStr, Value>,
    location: MemoryLocation,
    name: &str,
) -> Option<crate::value::ValueRef> {
    map.get_index_of(name).map(|offset| crate::value::ValueRef {
        location,
        offset,
        path: RefPath::new(),
    })
}
