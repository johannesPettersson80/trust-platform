//! The places of the project's crate's structures (specification 48 RU-29): the generated code's
//! type `Places` reads and writes each structure a Rust POU's value reaches where it lies, field by
//! field (`trust_plc::__private::PlaceOf`). An in-out of the structure and a reference to it read
//! and write it through `Places` at once, and a Rust POU's variable of it is read and written back
//! through it (RU-28). `Places` is the generated crate's own type, so the impls hold whichever
//! crate the generated structure lies in: layer 1's, when the project's crate calls Structured
//! Text (§9.2).

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use super::super::types::{Kind, Ty, Types};
use super::super::NativeError;
use super::rust_glue::{load, store, structure_path};

/// `Places` with its `impl trust_plc::__private::PlaceOf` of each structure of `wanted` and of each
/// structure their fields reach; `structures` gives the path of each structure of the crate by its
/// name in capitals.
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
            let place = format!("place.{}", member.field());
            let value = load(types, member.ty, &place, structures, &mut reached, 0)?;
            // The field as a raw identifier, which names a field that a Rust keyword spells
            // (`r#loop`) as it names any other: a structure of the crate comes from Rust, so its
            // fields are never `self`, `Self`, `super` or `crate`, which no raw identifier spells.
            fields.push(format!("r#{}: {value}", member.name));
            let statement = store(
                types,
                member.ty,
                &place,
                &format!("value.r#{}", member.name),
                structures,
                &mut reached,
                0,
            )?;
            let _ = writeln!(stores, "        {statement}");
        }
        work.extend(reached);
        let _ = write!(
            source,
            "impl ::trust_plc::__private::PlaceOf<{rust}, {path}> for Places {{\n    \
             #[inline]\n    fn load(place: &{rust}) -> {path} {{\n        {path} {{ {fields} }}\n    }}\n    \
             #[inline]\n    fn store(place: &{rust}, value: &{path}) {{\n{stores}    }}\n}}\n",
            rust = info.rust,
            fields = fields.join(", "),
        );
    }
    if !source.is_empty() {
        source.insert_str(
            0,
            "/// The places of the crate's structures (specification 48 RU-29).\npub struct Places;\n",
        );
    }
    Ok(source)
}
