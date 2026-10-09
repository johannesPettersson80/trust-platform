//! Reuse hosted I/O layout rules with declaration-derived references.

use super::*;
use crate::harness::{InstanceBinding, WildcardRequirement};
use crate::io::IoAddress;
use crate::memory::InstanceId;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn collect_source_io(&mut self) -> Result<(), BytecodeError> {
        let input = self
            .authoring
            .ok_or_else(|| invalid("source declarations missing"))?;
        for global in &input.globals {
            let reference = self
                .construction
                .bindings
                .global(&global.name)
                .cloned()
                .ok_or_else(|| invalid("global I/O reference missing"))?;
            let address = global
                .address
                .as_ref()
                .map(|text| IoAddress::parse(text))
                .transpose()
                .map_err(|_| invalid("invalid global I/O address"))?;
            self.bind_source_declaration(
                reference,
                global.type_id,
                address.as_ref(),
                &global.name,
            )?;
        }
        let roots = self.construction.roots.entries.clone();
        for root in roots {
            let (Some(instance), Some(template)) = (root.instance_owner_id, root.template_pou_id)
            else {
                continue;
            };
            let instance = InstanceId(instance);
            let mut declarations = Vec::new();
            if let Some(program) = input
                .program_defs
                .values()
                .find(|program| self.pou_ids.program_id(&program.name) == Some(template))
            {
                declarations.extend(
                    program
                        .vars
                        .iter()
                        .filter(|var| !var.external)
                        .map(|var| (var.name.clone(), var.type_id, var.address.clone())),
                );
            } else if let Some(fb) = self
                .runtime
                .function_blocks()
                .values()
                .find(|fb| self.pou_ids.function_block_id(&fb.name) == Some(template))
            {
                declarations.extend(
                    fb.params
                        .iter()
                        .map(|param| (param.name.clone(), param.type_id, param.address.clone())),
                );
                declarations.extend(
                    fb.vars
                        .iter()
                        .filter(|var| !var.external)
                        .map(|var| (var.name.clone(), var.type_id, var.address.clone())),
                );
            }
            for (name, ty, address) in declarations {
                let reference = self
                    .construction
                    .bindings
                    .instance(instance, &name)
                    .cloned()
                    .ok_or_else(|| invalid("instance I/O reference missing"))?;
                self.bind_source_declaration(reference, ty, address.as_ref(), &name)?;
            }
        }
        Ok(())
    }

    pub(in crate::bytecode::encoder) fn bind_source_declaration(
        &mut self,
        reference: ValueRef,
        ty: TypeId,
        address: Option<&IoAddress>,
        name: &SmolStr,
    ) -> Result<(), BytecodeError> {
        let mut bindings = Vec::new();
        if let Some(address) = address {
            if address.wildcard {
                self.construction.wildcards.push(WildcardRequirement {
                    name: name.clone(),
                    reference,
                    area: address.area,
                });
            } else {
                bindings.push(InstanceBinding {
                    reference,
                    type_id: ty,
                    address: address.clone(),
                    display_name: name.clone(),
                });
            }
        } else {
            crate::harness::collect_direct_field_bindings(
                self.runtime.registry(),
                &reference,
                ty,
                name,
                &mut self.construction.wildcards,
                &mut bindings,
            )
            .map_err(|error| BytecodeError::InvalidSection(error.to_string().into()))?;
        }
        for binding in bindings {
            crate::harness::bind_value_ref_to_address(
                &mut self.construction.io,
                self.runtime.registry(),
                binding.reference,
                binding.type_id,
                &binding.address,
                Some(binding.display_name),
            )
            .map_err(|error| BytecodeError::InvalidSection(error.to_string().into()))?;
        }
        Ok(())
    }
}
