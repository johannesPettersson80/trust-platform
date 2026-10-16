//! One staged physical patch and alias preflight; no destination-by-source quadratic loop.
use super::*;
use core::ops::Range;

pub(crate) struct PreparedPhysicalWrite {
    bits: Option<Range<u32>>,
    byte_start: u32,
    bytes: Vec<u8>,
    updates: Vec<(usize, Value)>,
}

impl StructValue {
    /// Build and validate a detached patch before any COW or installed-state mutation.
    pub(crate) fn prepare_physical_write(
        &self,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<Option<PreparedPhysicalWrite>, RuntimeError> {
        let Some(physical) = &self.physical else {
            return Ok(None);
        };
        if !path.is_empty() && read_struct_path(self, path).is_none() {
            return Ok(None);
        }
        let Some(selection) = physical.layout.selection(path) else {
            return Ok(Some(PreparedPhysicalWrite {
                bits: None,
                byte_start: 0,
                bytes: Vec::new(),
                updates: Vec::new(),
            }));
        };
        validate_selected_children(self, &physical.layout, &selection, path, input)?;
        let byte_start = selection.bits.start / 8;
        let byte_end = selection.bits.end.div_ceil(8);
        let range = usize::try_from(byte_start).map_err(|_| RuntimeError::Overflow)?
            ..usize::try_from(byte_end).map_err(|_| RuntimeError::Overflow)?;
        let original = physical
            .bytes
            .get(range)
            .ok_or(RuntimeError::TypeMismatch)?;
        let mut bytes = reserved(original.len())?;
        bytes.extend_from_slice(original);
        // Aggregate assignment copies the entire selected backing, including gaps.
        // A raw construction input has zero-valued gaps; an admitted input carries them.
        if !selection.children.is_empty() || path.is_empty() {
            for bit in selection.bits.clone() {
                write_bits(&mut bytes, bit - byte_start * 8, 1, 0);
            }
            copy_input_backing(
                &physical.layout,
                &selection,
                path,
                input,
                byte_start,
                &mut bytes,
            )?;
        }
        for &i in &physical.layout.path_order[selection.leaves.clone()] {
            let leaf = &physical.layout.leaves[i];
            let suffix = path_suffix(&leaf.path, path).ok_or(RuntimeError::TypeMismatch)?;
            let source =
                read_value_path_borrowed(input, suffix).ok_or(RuntimeError::TypeMismatch)?;
            let template = read_struct_path(self, &leaf.path).ok_or(RuntimeError::TypeMismatch)?;
            let bits = scalar::checked_bits(source, template, leaf, physical.layout.abi)?;
            write_bits(&mut bytes, leaf.offset - byte_start * 8, leaf.width, bits);
        }
        // A supplied aggregate must already describe one coherent physical value.
        // Fresh default construction has its separate initialization owner; a
        // checked assignment must never resolve conflicting input aliases by order.
        for &i in &physical.layout.path_order[selection.leaves.clone()] {
            let leaf = &physical.layout.leaves[i];
            let suffix = path_suffix(&leaf.path, path).ok_or(RuntimeError::TypeMismatch)?;
            let source =
                read_value_path_borrowed(input, suffix).ok_or(RuntimeError::TypeMismatch)?;
            let template = read_struct_path(self, &leaf.path).ok_or(RuntimeError::TypeMismatch)?;
            let incoming = scalar::checked_bits(source, template, leaf, physical.layout.abi)?;
            let mask = u64::MAX >> (64 - leaf.width);
            if incoming & mask != read_bits(&bytes, leaf.offset - byte_start * 8, leaf.width) {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        let updates = decode_updates(self, physical, &selection.bits, byte_start, &bytes)?;
        Ok(Some(PreparedPhysicalWrite {
            bits: Some(selection.bits),
            byte_start,
            bytes,
            updates,
        }))
    }

    /// Every path, range, value and alias was admitted by prepare_physical_write.
    /// Internal member writes deliberately preserve the immutable layout marker.
    pub(crate) fn commit_physical_write(
        &mut self,
        path: &[RefSegment],
        input: Value,
        prepared: PreparedPhysicalWrite,
    ) -> bool {
        if path.is_empty() {
            let Value::Struct(input) = input else {
                return false;
            };
            self.fields = match Arc::try_unwrap(input) {
                Ok(input) => input.fields,
                Err(input) => input.fields.clone(),
            };
        } else if !write_struct_path_raw(self, path, input) {
            return false;
        }
        if let Some(physical) = &mut self.physical {
            let start = prepared.byte_start as usize;
            if let Some(destination) = physical.bytes.get_mut(start..start + prepared.bytes.len()) {
                destination.copy_from_slice(&prepared.bytes);
            }
        }
        for (index, value) in prepared.updates {
            let Some(physical) = &self.physical else {
                return false;
            };
            // Borrow immutable metadata independently while mutating logical fields.
            let layout = Arc::clone(&physical.layout);
            if !write_struct_path_raw(self, &layout.leaves[index].path, value) {
                return false;
            }
        }
        if let Some(bits) = prepared.bits {
            self.refresh_child_backings(&bits);
        }
        true
    }

    fn refresh_child_backings(&mut self, bits: &Range<u32>) {
        let Some(physical) = self.physical.take() else {
            return;
        };
        for i in physical.layout.affected_children(bits) {
            let child = &physical.layout.children[i];
            let start = child.offset.max(bits.start);
            let end = (child.offset + child.bits).min(bits.end);
            if start >= end {
                continue;
            }
            if let Some(Value::Struct(value)) = struct_path_mut(self, &child.path)
                && let Some(target) = Arc::make_mut(value).physical.as_mut()
            {
                for bit in start..end {
                    write_bits(
                        &mut target.bytes,
                        bit - child.offset,
                        1,
                        read_bits(&physical.bytes, bit, 1),
                    );
                }
            }
        }
        self.physical = Some(physical);
    }
}

impl PreparedPhysicalWrite {
    pub(crate) fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        for (_, value) in &self.updates {
            check(value)?;
        }
        Ok(())
    }
}

pub(super) fn initialize(
    value: &mut StructValue,
    layout: Arc<PhysicalLayout>,
    strict: bool,
    seed: Option<&PhysicalValue>,
) -> Result<(), RuntimeError> {
    let count = layout.byte_len()?;
    let mut bytes = reserved(count)?;
    bytes.resize(count, 0);
    if let Some(seed) = seed {
        if seed.bytes.len() != bytes.len() {
            return Err(RuntimeError::TypeMismatch);
        }
        bytes.copy_from_slice(&seed.bytes);
    }
    let selection = layout.selection(&[]);
    if let Some(selection) = &selection {
        // Incoming nested values can already own admitted gaps from construction/retain.
        for &i in &layout.child_path_order[selection.children.clone()] {
            let child = &layout.children[i];
            if let Some(Value::Struct(source)) = read_struct_path(value, &child.path)
                && let Some(source) = &source.physical
            {
                if source.layout.bits != child.bits {
                    return Err(RuntimeError::TypeMismatch);
                }
                copy_bits(&mut bytes, child.offset, &source.bytes, 0, child.bits);
            }
        }
    }
    for leaf in &layout.leaves {
        let input = read_struct_path(value, &leaf.path).ok_or(RuntimeError::TypeMismatch)?;
        let bits = scalar::checked_bits(input, input, leaf, layout.abi)?;
        write_bits(&mut bytes, leaf.offset, leaf.width, bits);
    }
    let physical = PhysicalValue { layout, bytes };
    let updates = decode_updates(
        value,
        &physical,
        &(0..physical.layout.bits),
        0,
        &physical.bytes,
    )?;
    if strict {
        for (i, reconstructed) in &updates {
            let incoming = read_struct_path(value, &physical.layout.leaves[*i].path)
                .ok_or(RuntimeError::TypeMismatch)?;
            let leaf = &physical.layout.leaves[*i];
            if scalar::checked_bits(incoming, incoming, leaf, physical.layout.abi)?
                != scalar::bits(reconstructed).ok_or(RuntimeError::TypeMismatch)?
            {
                return Err(RuntimeError::TypeMismatch);
            }
        }
    }
    value.physical = Some(physical);
    for (i, replacement) in updates {
        let layout = Arc::clone(
            &value
                .physical
                .as_ref()
                .ok_or(RuntimeError::TypeMismatch)?
                .layout,
        );
        if !write_struct_path_raw(value, &layout.leaves[i].path, replacement) {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    let bits = value
        .physical
        .as_ref()
        .ok_or(RuntimeError::TypeMismatch)?
        .layout
        .bits;
    value.refresh_child_backings(&(0..bits));
    Ok(())
}

fn validate_selected_children(
    target: &StructValue,
    layout: &PhysicalLayout,
    selection: &index::Selection,
    path: &[RefSegment],
    input: &Value,
) -> Result<(), RuntimeError> {
    if path.is_empty() {
        let Value::Struct(source) = input else {
            return Err(RuntimeError::TypeMismatch);
        };
        if source.type_name() != target.type_name()
            || source.fields.len() != target.fields.len()
            || target
                .fields
                .keys()
                .any(|name| !source.fields.contains_key(name))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(physical) = &source.physical
            && !physical.layout.storage_compatible(layout)
        {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    for &i in &layout.child_path_order[selection.children.clone()] {
        let child = &layout.children[i];
        let suffix = path_suffix(&child.path, path).ok_or(RuntimeError::TypeMismatch)?;
        let Some(Value::Struct(source)) = read_value_path_borrowed(input, suffix) else {
            return Err(RuntimeError::TypeMismatch);
        };
        let Some(Value::Struct(expected)) = read_struct_path(target, &child.path) else {
            return Err(RuntimeError::TypeMismatch);
        };
        // Engine normalization attaches this admitted layout before a write. A raw
        // member mutation clears its marker and must be admitted again first.
        match (&source.physical, &expected.physical) {
            (Some(source), Some(expected))
                if source.layout.storage_compatible(&expected.layout) => {}
            _ => return Err(RuntimeError::TypeMismatch),
        }
    }
    Ok(())
}

fn copy_input_backing(
    layout: &PhysicalLayout,
    selection: &index::Selection,
    path: &[RefSegment],
    input: &Value,
    byte_start: u32,
    bytes: &mut [u8],
) -> Result<(), RuntimeError> {
    if path.is_empty()
        && let Value::Struct(source) = input
        && let Some(source) = &source.physical
    {
        copy_bits(bytes, 0, &source.bytes, 0, layout.bits);
        return Ok(());
    }
    let mut previous: Option<&[RefSegment]> = None;
    for &i in &layout.child_path_order[selection.children.clone()] {
        let child = &layout.children[i];
        if previous.is_some_and(|previous| path_suffix(&child.path, previous).is_some()) {
            continue;
        }
        let suffix = path_suffix(&child.path, path).ok_or(RuntimeError::TypeMismatch)?;
        if let Some(Value::Struct(source)) = read_value_path_borrowed(input, suffix)
            && let Some(source) = &source.physical
        {
            copy_bits(
                bytes,
                child.offset - byte_start * 8,
                &source.bytes,
                0,
                child.bits,
            );
            previous = Some(&child.path);
        }
    }
    Ok(())
}

fn decode_updates(
    target: &StructValue,
    physical: &PhysicalValue,
    changed: &Range<u32>,
    byte_start: u32,
    patch: &[u8],
) -> Result<Vec<(usize, Value)>, RuntimeError> {
    let candidates = physical.layout.affected_leaves(changed);
    let mut result = reserved(candidates.len())?;
    for &i in &physical.layout.bit_order[candidates] {
        let leaf = &physical.layout.leaves[i];
        if leaf.offset + u32::from(leaf.width) <= changed.start {
            continue;
        }
        let template = read_struct_path(target, &leaf.path).ok_or(RuntimeError::TypeMismatch)?;
        let mut bits = 0;
        for bit in 0..u32::from(leaf.width) {
            let position = leaf.offset + bit;
            let value =
                if position / 8 >= byte_start && position / 8 - byte_start < patch.len() as u32 {
                    read_bits(patch, position - byte_start * 8, 1)
                } else {
                    read_bits(&physical.bytes, position, 1)
                };
            bits |= value << bit;
        }
        scalar::validate(template, bits, leaf)?;
        let value = scalar::value(template, bits, leaf).ok_or(RuntimeError::TypeMismatch)?;
        result.push((i, value));
    }
    Ok(result)
}

fn copy_bits(target: &mut [u8], start: u32, source: &[u8], source_start: u32, length: u32) {
    for bit in 0..length {
        write_bits(
            target,
            start + bit,
            1,
            read_bits(source, source_start + bit, 1),
        );
    }
}

fn reserved<T>(count: usize) -> Result<Vec<T>, RuntimeError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| RuntimeError::Overflow)?;
    Ok(result)
}
