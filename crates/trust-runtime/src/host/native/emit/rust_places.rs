//! The places of the project's crate's structures (specification 48 RU-29): the generated code's
//! type of each structure a Rust POU's value reaches is the place of its Rust struct,
//! `trust_plc::Place`, which reads and writes it where it lies, field by field. An in-out of the
//! structure and a reference to it read and write it through that place at once, and a Rust POU's
//! variable of it is read and written back through it (RU-28).

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use super::super::types::{Kind, Ty, Types};
use super::super::NativeError;
use super::rust_glue::{load, store, structure_path};

/// The `impl trust_plc::Place` of each structure of `wanted` and of each structure their fields
/// reach; `structures` gives the path of each structure of the crate by its name in capitals.
///
/// # Errors
///
/// [`NativeError::Unsupported`] for a field of a type the SDK does not carry, and for a structure
/// that is none of the crate's.
pub(super) fn places(
    types: &Types<'_>,
    wanted: &BTreeSet<Ty>,
    structures: &HashMap<String, String>,
) -> Result<String, NativeError> {
    let mut source = String::new();
    let mut work: Vec<Ty> = wanted.iter().copied().collect();
    let mut written = BTreeSet::new();
    while let Some(ty) = work.pop() {
        if !written.insert(ty) {
            continue;
        }
        let info = types.info(ty);
        let Kind::Struct { members } = &info.kind else {
            continue;
        };
        let path = structure_path(structures, &info.name)?;
        let mut reached = BTreeSet::new();
        let mut fields = Vec::with_capacity(members.len());
        let mut stores = String::new();
        for member in members {
            let place = format!("self.{}", member.field());
            let value = load(types, member.ty, &place, structures, &mut reached, 0)?;
            fields.push(format!("{}: {value}", member.name));
            let statement = store(
                types,
                member.ty,
                &place,
                &format!("value.{}", member.name),
                structures,
                &mut reached,
                0,
            )?;
            let _ = writeln!(stores, "        {statement}");
        }
        work.extend(reached);
        let _ = write!(
            source,
            "impl ::trust_plc::Place<{path}> for {rust} {{\n    \
             #[inline]\n    fn load(&self) -> {path} {{\n        {path} {{ {fields} }}\n    }}\n    \
             #[inline]\n    fn store(&self, value: &{path}) {{\n{stores}    }}\n}}\n",
            rust = info.rust,
            fields = fields.join(", "),
        );
    }
    Ok(source)
}
