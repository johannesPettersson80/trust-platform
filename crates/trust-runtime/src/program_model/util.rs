use smol_str::SmolStr;

pub fn static_storage_name(owner: &SmolStr, name: &SmolStr) -> SmolStr {
    SmolStr::new(format!("__STAT::{owner}::{name}"))
}

pub fn method_static_storage_owner(owner: &SmolStr, method: &SmolStr) -> SmolStr {
    SmolStr::new(format!("{owner}::{method}"))
}

pub fn property_setter_method_name(property: &SmolStr) -> SmolStr {
    SmolStr::new(format!("__set_{property}"))
}
