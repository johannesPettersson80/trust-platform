use alloc::format;
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

#[cfg(test)]
mod tests {
    use super::{method_static_storage_owner, property_setter_method_name, static_storage_name};
    use smol_str::SmolStr;

    #[test]
    fn static_storage_names_keep_existing_prefix_contract() {
        assert_eq!(
            static_storage_name(&SmolStr::new("Owner"), &SmolStr::new("local")),
            "__STAT::Owner::local"
        );
        assert_eq!(
            method_static_storage_owner(&SmolStr::new("Owner"), &SmolStr::new("M")),
            "Owner::M"
        );
    }

    #[test]
    fn property_setter_names_keep_hidden_prefix_contract() {
        assert_eq!(
            property_setter_method_name(&SmolStr::new("Level")),
            "__set_Level"
        );
    }
}
