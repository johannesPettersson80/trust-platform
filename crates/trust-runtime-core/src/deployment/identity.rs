//! The identities of a deployment record (specification 34 §5.1.1).

use crate::bytecode::{
    BytecodeModule, ConstructionRoot, Field, PouEntry, SectionData, SectionId, StorageDeclaration,
    StorageOwner, StorageRole, TypeData, TypeEntry,
};
use crate::error::RuntimeError;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};
use smol_str::SmolStr;

/// The SHA-256 of `bytes` (FIPS 180-4).
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The SHA-256 of the bytes of `parts` one after the other (FIPS 180-4): the digest of their
/// concatenation, worked out part by part without joining them. A reply that a chip holds in
/// parts of 4,096 bytes is hashed this way, because its heap has no block for the whole reply
/// (specification 34, PORT-05).
#[must_use]
pub fn sha256_parts<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// The SHA-256 of bytes that arrive in pieces over time (FIPS 180-4): the digest of their
/// concatenation, worked out piece by piece. A chip works out the SHA-256 of an upload that it writes
/// into its program store this way, part by part as each part is written, because it holds no more
/// of the upload than one part (specification 34 §5.1).
#[derive(Clone, Default)]
pub struct Sha256Stream(Sha256);

impl Sha256Stream {
    /// The SHA-256 of no bytes yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` after the bytes added before.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The SHA-256 of every byte added, in the order added.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

impl core::fmt::Debug for Sha256Stream {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Sha256Stream")
    }
}

/// The configuration identity: SHA-256 over the project's `runtime.toml` and then its
/// `io.toml`, each as its file name, a zero byte, and either a byte 1, its length as eight
/// little-endian bytes and its bytes, or a byte 0 when the project has no such file.
#[must_use]
pub fn configuration_identity(runtime_toml: Option<&[u8]>, io_toml: Option<&[u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for (name, content) in [("runtime.toml", runtime_toml), ("io.toml", io_toml)] {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        match content {
            Some(bytes) => {
                hasher.update([1u8]);
                hasher.update(super::wide(bytes.len()).to_le_bytes());
                hasher.update(bytes);
            }
            None => hasher.update([0u8]),
        }
    }
    hasher.finalize().into()
}

/// The retain-schema identity of a decoded program: SHA-256 over the declarations a checkpoint
/// keeps, in the order it keeps them (every retained global, then the retained variables of each
/// program instance), each with its checkpoint name, its retention policy and its type, followed
/// by TYPE_TABLE with every name spelled out.
///
/// # Errors
///
/// A declaration, root, type or name refers to a record the program does not have.
pub fn retain_schema_identity(module: &BytecodeModule) -> Result<[u8; 32], RuntimeError> {
    let strings: &[SmolStr] = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table.entries.as_slice(),
        _ => &[],
    };
    let declarations: &[StorageDeclaration] = match module.section(SectionId::StorageLayout) {
        Some(SectionData::StorageLayout(layout)) => layout.entries.as_slice(),
        _ => &[],
    };
    let roots: &[ConstructionRoot] = match module.section(SectionId::ConstructionRoots) {
        Some(SectionData::ConstructionRoots(roots)) => roots.entries.as_slice(),
        _ => &[],
    };
    let types: &[TypeEntry] = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table.entries.as_slice(),
        _ => &[],
    };
    let pous: &[PouEntry] = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index.entries.as_slice(),
        _ => &[],
    };
    let schema = Schema { strings, pous };
    let mut hasher = Sha256::new();
    // A checkpoint keeps the retained globals first, by name.
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.owner == StorageOwner::Global && retained(declaration))
    {
        hasher.update_framed(schema.name(declaration.name_idx)?.as_bytes());
        schema.declaration(&mut hasher, declaration);
    }
    // Then the retained variables of each program instance, as `@program/<instance>/<name>`.
    for root in roots.iter().filter(|root| root.parent_root_idx.is_none()) {
        let instance = declarations
            .get(root.declaration_idx as usize)
            .ok_or_else(|| invalid("construction root"))?;
        if instance.role != StorageRole::ProgramRoot {
            continue;
        }
        let instance_name = schema.name(instance.name_idx)?;
        for declaration in declarations.iter().filter(|declaration| {
            declaration.owner == StorageOwner::Instance
                && declaration.owner_pou_id == root.template_pou_id
                && retained(declaration)
        }) {
            let name = schema.name(declaration.name_idx)?;
            let mut key = Vec::new();
            key.extend_from_slice(b"@program/");
            key.extend_from_slice(instance_name.as_bytes());
            key.push(b'/');
            key.extend_from_slice(name.as_bytes());
            hasher.update_framed(&key);
            schema.declaration(&mut hasher, declaration);
        }
    }
    hasher.update(super::wide(types.len()).to_le_bytes());
    for entry in types {
        schema.type_entry(&mut hasher, entry)?;
    }
    Ok(hasher.finalize().into())
}

/// A declaration whose value a checkpoint keeps: a RETAIN or PERSISTENT variable.
fn retained(declaration: &StorageDeclaration) -> bool {
    declaration.role == StorageRole::Variable && declaration.is_retained()
}

fn invalid(what: &'static str) -> RuntimeError {
    RuntimeError::InvalidBytecodeMetadata(SmolStr::new_static(what))
}

/// The tables a retain schema names.
struct Schema<'a> {
    strings: &'a [SmolStr],
    pous: &'a [PouEntry],
}

impl Schema<'_> {
    fn name(&self, index: u32) -> Result<&SmolStr, RuntimeError> {
        self.strings
            .get(index as usize)
            .ok_or_else(|| invalid("string index"))
    }

    fn optional_name(&self, hasher: &mut Sha256, index: Option<u32>) -> Result<(), RuntimeError> {
        match index {
            Some(index) => {
                hasher.update([1u8]);
                hasher.update_framed(self.name(index)?.as_bytes());
            }
            None => hasher.update([0u8]),
        }
        Ok(())
    }

    /// The retention policy and the type of a retained declaration.
    fn declaration(&self, hasher: &mut Sha256, declaration: &StorageDeclaration) {
        hasher.update([declaration.retain]);
        hasher.update(declaration.type_id.unwrap_or(u32::MAX).to_le_bytes());
    }

    /// One TYPE_TABLE record with its names spelled out; type references stay indices into the
    /// table, which follows in full.
    fn type_entry(&self, hasher: &mut Sha256, entry: &TypeEntry) -> Result<(), RuntimeError> {
        hasher.update([entry.kind as u8]);
        self.optional_name(hasher, entry.name_idx)?;
        match &entry.data {
            TypeData::Primitive {
                prim_id,
                max_length,
            } => {
                hasher.update([0u8]);
                hasher.update(prim_id.to_le_bytes());
                hasher.update(max_length.to_le_bytes());
            }
            TypeData::Array { elem_type_id, dims } => {
                hasher.update([1u8]);
                hasher.update(elem_type_id.to_le_bytes());
                hasher.update(super::wide(dims.len()).to_le_bytes());
                for (lower, upper) in dims {
                    hasher.update(lower.to_le_bytes());
                    hasher.update(upper.to_le_bytes());
                }
            }
            TypeData::Struct { fields } => self.fields(hasher, 2, fields)?,
            TypeData::Union { fields } => self.fields(hasher, 7, fields)?,
            TypeData::Enum {
                base_type_id,
                variants,
            } => {
                hasher.update([3u8]);
                hasher.update(base_type_id.to_le_bytes());
                hasher.update(super::wide(variants.len()).to_le_bytes());
                for variant in variants {
                    hasher.update_framed(self.name(variant.name_idx)?.as_bytes());
                    hasher.update(variant.value.to_le_bytes());
                }
            }
            TypeData::Alias { target_type_id } => {
                hasher.update([4u8]);
                hasher.update(target_type_id.to_le_bytes());
            }
            TypeData::Subrange {
                base_type_id,
                lower,
                upper,
            } => {
                hasher.update([5u8]);
                hasher.update(base_type_id.to_le_bytes());
                hasher.update(lower.to_le_bytes());
                hasher.update(upper.to_le_bytes());
            }
            TypeData::Reference { target_type_id } => {
                hasher.update([6u8]);
                hasher.update(target_type_id.to_le_bytes());
            }
            TypeData::Pou { pou_id } => {
                hasher.update([8u8]);
                let pou = self
                    .pous
                    .iter()
                    .find(|pou| pou.id == *pou_id)
                    .ok_or_else(|| invalid("POU type"))?;
                hasher.update_framed(self.name(pou.name_idx)?.as_bytes());
            }
            TypeData::Interface { methods } => {
                hasher.update([9u8]);
                hasher.update(super::wide(methods.len()).to_le_bytes());
                for method in methods {
                    hasher.update_framed(self.name(method.name_idx)?.as_bytes());
                    hasher.update(method.slot.to_le_bytes());
                }
            }
        }
        Ok(())
    }

    /// The fields of a structure (`tag` 2) or a union (`tag` 7).
    fn fields(&self, hasher: &mut Sha256, tag: u8, fields: &[Field]) -> Result<(), RuntimeError> {
        hasher.update([tag]);
        hasher.update(super::wide(fields.len()).to_le_bytes());
        for field in fields {
            hasher.update_framed(self.name(field.name_idx)?.as_bytes());
            hasher.update(field.type_id.to_le_bytes());
            match field.bit_offset {
                Some(offset) => {
                    hasher.update([1u8]);
                    hasher.update(offset.to_le_bytes());
                }
                None => hasher.update([0u8]),
            }
        }
        Ok(())
    }
}

/// Bytes hashed with their length first, so that two fields never run together.
trait Framed {
    fn update_framed(&mut self, bytes: &[u8]);
}

impl Framed for Sha256 {
    fn update_framed(&mut self, bytes: &[u8]) {
        self.update(super::wide(bytes.len()).to_le_bytes());
        self.update(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest of `hex`, 64 hexadecimal digits.
    fn digest(hex: &str) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        bytes
    }

    /// FIPS 180-4's example "abc" (Appendix B.1 of FIPS 180-2) fed as "a" and then "bc" has the
    /// digest of the whole message, and no bytes have the digest of the empty message. Catches a
    /// stream that restarts its state at each piece or pads each piece on its own.
    #[test]
    fn a_stream_of_pieces_has_the_digest_of_their_concatenation() {
        let mut stream = Sha256Stream::new();
        stream.update(b"a");
        stream.update(b"bc");
        assert_eq!(
            stream.finish(),
            digest("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            Sha256Stream::new().finish(),
            digest("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }
}
