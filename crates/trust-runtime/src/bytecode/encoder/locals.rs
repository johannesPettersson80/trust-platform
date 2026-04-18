use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::memory::{FrameId, MemoryLocation};
use crate::value::{RefPath, ValueRef};

use super::util::{count_for_loops, normalize_name};
use super::{BytecodeEncoder, BytecodeError, LocalScope};
use crate::program_model::Param;
use crate::program_model::{Stmt, VarDef};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn alloc_local_frame_id(&mut self) -> FrameId {
        let id = FrameId(self.next_local_frame_id);
        self.next_local_frame_id = self.next_local_frame_id.saturating_add(1);
        id
    }

    pub(super) fn local_scope_for_body(
        &mut self,
        return_name: Option<&SmolStr>,
        params: &[Param],
        locals: &[VarDef],
        body: &[Stmt],
    ) -> Result<LocalScope, BytecodeError> {
        let mut names = Vec::new();
        if let Some(name) = return_name {
            names.push(name.clone());
        }
        for param in params {
            names.push(param.name.clone());
        }
        for local in locals {
            if local.external {
                continue;
            }
            names.push(local.name.clone());
        }
        let for_loop_count = count_for_loops(body);
        let for_temp_pairs = self.alloc_for_temp_pairs(&names, for_loop_count);
        for (end, step) in &for_temp_pairs {
            names.push(end.clone());
            names.push(step.clone());
        }
        self.build_local_scope(names, for_temp_pairs)
    }

    fn alloc_for_temp_pairs(&self, existing: &[SmolStr], count: usize) -> Vec<(SmolStr, SmolStr)> {
        if count == 0 {
            return Vec::new();
        }
        let mut used: HashSet<SmolStr> =
            existing.iter().map(normalize_name).collect::<HashSet<_>>();
        let mut pairs = Vec::with_capacity(count);
        for idx in 0..count {
            let end_name = self.unique_temp_name("__st_rt_for_end", idx, &mut used);
            let step_name = self.unique_temp_name("__st_rt_for_step", idx, &mut used);
            pairs.push((end_name, step_name));
        }
        pairs
    }

    fn unique_temp_name(&self, prefix: &str, idx: usize, used: &mut HashSet<SmolStr>) -> SmolStr {
        let mut attempt = 0usize;
        loop {
            let name = if attempt == 0 {
                SmolStr::new(format!("{prefix}_{idx}"))
            } else {
                SmolStr::new(format!("{prefix}_{idx}_{attempt}"))
            };
            let key = normalize_name(&name);
            if !used.contains(&key) {
                used.insert(key);
                return name;
            }
            attempt = attempt.saturating_add(1);
        }
    }

    fn build_local_scope(
        &mut self,
        names: Vec<SmolStr>,
        for_temp_pairs: Vec<(SmolStr, SmolStr)>,
    ) -> Result<LocalScope, BytecodeError> {
        let local_ref_start = self.ref_entries.len() as u32;
        if names.is_empty() {
            return Ok(LocalScope {
                locals: HashMap::new(),
                local_ref_start,
                local_ref_count: 0,
                for_temp_pairs,
            });
        }
        let frame_id = self.alloc_local_frame_id();
        let mut locals = HashMap::new();
        let mut seen = HashSet::new();
        let mut offset: usize = 0;
        for name in names {
            let key = normalize_name(&name);
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key.clone());
            let value_ref = ValueRef {
                location: MemoryLocation::Local(frame_id),
                offset,
                path: RefPath::new(),
            };
            self.ref_index_for(&value_ref)?;
            locals.insert(key, value_ref);
            offset = offset.saturating_add(1);
        }
        let local_ref_count = self
            .ref_entries
            .len()
            .saturating_sub(local_ref_start as usize) as u32;
        Ok(LocalScope {
            locals,
            local_ref_start,
            local_ref_count,
            for_temp_pairs,
        })
    }
}
