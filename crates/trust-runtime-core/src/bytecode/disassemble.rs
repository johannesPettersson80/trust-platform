//! Deterministic text inspection of validated legacy and source-free containers.

use super::*;
use alloc::{format, string::String};

impl BytecodeModuleView<'_> {
    /// Inspect metadata and ordinary/initializer instructions without executing them.
    /// This authoring/debug operation allocates text and is not a scan-path operation.
    pub fn disassemble(&self) -> Result<String, BytecodeError> {
        self.validate()?;
        let mut text = format!(
            "STBC {}.{} flags=0x{:08X}\n",
            self.version.major, self.version.minor, self.flags
        );
        let code = match self.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(v)) => v,
            _ => return Err(BytecodeError::MissingSection("POU_BODIES".into())),
        };
        for section in self.sections {
            text.push_str(&format!("SECTION 0x{:04X}\n", section.id));
            match &section.data {
                SectionData::PouIndex(index) => {
                    for entry in &index.entries {
                        text.push_str(&format!("  POU {} {}\n", entry.id, entry.kind as u8));
                        write_range(&mut text, code, entry.code_offset, entry.code_length)?;
                    }
                }
                SectionData::StorageLayout(layout) => {
                    for (id, entry) in layout.entries.iter().enumerate() {
                        text.push_str(&format!("  DECL {id} {}\n", storage_declaration(entry)));
                    }
                }
                SectionData::ConstructionRoots(roots) => {
                    for (id, entry) in roots.entries.iter().enumerate() {
                        text.push_str(&format!("  ROOT {id} {}\n", construction_root(entry)));
                    }
                }
                SectionData::AccessBindings(bindings) => {
                    for entry in &bindings.entries {
                        text.push_str(&format!("  ACCESS {}\n", access_binding_entry(entry)));
                    }
                }
                SectionData::Initializers(index) => {
                    for (id, entry) in index.entries.iter().enumerate() {
                        text.push_str(&format!("  INIT {id} {}\n", initializer_entry(entry)));
                        write_range(&mut text, code, entry.code_offset, entry.code_length)?;
                    }
                }
                _ => {}
            }
        }
        Ok(text)
    }
}

impl BytecodeModule {
    /// Disassemble the portable container without runtime construction.
    pub fn disassemble(&self) -> Result<String, BytecodeError> {
        self.view().disassemble()
    }
}

fn write_range(
    text: &mut String,
    code: &[u8],
    start: u32,
    length: u32,
) -> Result<(), BytecodeError> {
    let end = start
        .checked_add(length)
        .ok_or(RejectionReason::PouCodeRangeOverflow)?;
    let bytes = code
        .get(start as usize..end as usize)
        .ok_or(RejectionReason::PouCodeOutOfBounds)?;
    let mut reader = BytecodeReader::new(bytes);
    while reader.remaining() != 0 {
        let pc = start as usize + reader.pos();
        let opcode = reader.read_u8()?;
        let width =
            crate::vm::opcode_operand_len(opcode).ok_or(BytecodeError::InvalidOpcode(opcode))?;
        text.push_str(&format!("    {pc:08X}: {opcode:02X}"));
        for byte in reader.read_bytes(width)? {
            text.push_str(&format!(" {byte:02X}"));
        }
        text.push('\n');
    }
    Ok(())
}

fn storage_declaration(entry: &StorageDeclaration) -> String {
    format!("owner={} role={} retain={} flags={} owner_pou_id={} name_idx={} type_id={} slot={} ref_idx={} default_const_idx={} construction_nodes={} related_declaration_idx={} source_name_idx={}",
        storage_owner(entry.owner),
        storage_role(entry.role),
        entry.retain,
        entry.flags,
        optional(entry.owner_pou_id),
        entry.name_idx,
        optional(entry.type_id),
        entry.slot,
        optional(entry.ref_idx),
        optional(entry.default_const_idx),
        entry.construction_nodes,
        optional(entry.related_declaration_idx),
        optional(entry.source_name_idx)
    )
}

fn construction_root(entry: &ConstructionRoot) -> String {
    format!("declaration_idx={} binding_ref_idx={} instance_owner_id={} parent_root_idx={} template_pou_id={} flags={}",
        entry.declaration_idx,
        optional(entry.binding_ref_idx),
        optional(entry.instance_owner_id),
        optional(entry.parent_root_idx),
        optional(entry.template_pou_id),
        entry.flags
    )
}

fn access_binding_entry(entry: &AccessBindingEntry) -> String {
    format!(
        "name_idx={} type_id={} ref_idx={} partial_kind={} flags={} reserved={} partial_index={}",
        entry.name_idx,
        entry.type_id,
        entry.ref_idx,
        entry.partial_kind,
        entry.flags,
        entry.reserved,
        entry.partial_index
    )
}

fn initializer_entry(entry: &InitializerEntry) -> String {
    format!("declaration_idx={} owner_pou_id={} result_ref_idx={} code_offset={} code_length={} visible_local_count={} visible_static_count={} phase={} once={} stage={} trigger={} target_idx={} partial_kind={} target_kind={} target_reserved=[{:02X},{:02X}] partial_index={} context_initializer_idx={} recipe_type_id={} recipe_member_idx={} body_kind={} recipe_reserved=[{:02X},{:02X},{:02X}]",
        optional(entry.declaration_idx),
        optional(entry.owner_pou_id),
        entry.result_ref_idx,
        entry.code_offset,
        entry.code_length,
        entry.visible_local_count,
        entry.visible_static_count,
        initialization_phase(entry.phase),
        initialization_once(entry.once),
        initialization_stage(entry.stage),
        initialization_trigger(entry.trigger),
        optional(entry.target_idx),
        entry.partial_kind,
        initialization_target(entry.target_kind),
        entry.target_reserved[0],
        entry.target_reserved[1],
        entry.partial_index,
        optional(entry.context_initializer_idx),
        optional(entry.recipe_type_id),
        optional(entry.recipe_member_idx),
        initializer_body_kind(entry.body_kind),
        entry.recipe_reserved[0],
        entry.recipe_reserved[1],
        entry.recipe_reserved[2]
    )
}

fn optional(value: Option<u32>) -> String {
    value.map_or_else(|| "none".into(), |value| format!("{value}"))
}

fn storage_owner(value: StorageOwner) -> &'static str {
    match value {
        StorageOwner::Global => "Global",
        StorageOwner::Instance => "Instance",
        StorageOwner::Frame => "Frame",
    }
}

fn storage_role(value: StorageRole) -> &'static str {
    match value {
        StorageRole::Variable => "Variable",
        StorageRole::ProgramRoot => "ProgramRoot",
        StorageRole::Return => "Return",
        StorageRole::Parameter => "Parameter",
        StorageRole::Static => "Static",
        StorageRole::External => "External",
        StorageRole::Scratch => "Scratch",
        StorageRole::EdgePhase => "EdgePhase",
        StorageRole::NativeState => "NativeState",
    }
}

fn initialization_phase(value: InitializationPhase) -> &'static str {
    match value {
        InitializationPhase::Resource => "Resource",
        InitializationPhase::Instance => "Instance",
        InitializationPhase::Frame => "Frame",
        InitializationPhase::Static => "Static",
        InitializationPhase::Return => "Return",
        InitializationPhase::Configuration => "Configuration",
        InitializationPhase::Parameter => "Parameter",
        InitializationPhase::ValueDefault => "ValueDefault",
    }
}

fn initialization_once(value: InitializationOnce) -> &'static str {
    match value {
        InitializationOnce::None => "None",
        InitializationOnce::Module => "Module",
        InitializationOnce::Instance => "Instance",
    }
}

fn initialization_stage(value: InitializationStage) -> &'static str {
    match value {
        InitializationStage::Default => "Default",
        InitializationStage::Explicit => "Explicit",
    }
}

fn initialization_trigger(value: InitializationTrigger) -> &'static str {
    match value {
        InitializationTrigger::Ordinary => "Ordinary",
        InitializationTrigger::AfterRestart => "AfterRestart",
    }
}

fn initialization_target(value: InitializationTarget) -> &'static str {
    match value {
        InitializationTarget::Declaration => "Declaration",
        InitializationTarget::Reference => "Reference",
        InitializationTarget::DirectIo => "DirectIo",
    }
}

fn initializer_body_kind(value: InitializerBodyKind) -> &'static str {
    match value {
        InitializerBodyKind::Action => "Action",
        InitializerBodyKind::TypeDefault => "TypeDefault",
        InitializerBodyKind::MemberDefault => "MemberDefault",
    }
}
