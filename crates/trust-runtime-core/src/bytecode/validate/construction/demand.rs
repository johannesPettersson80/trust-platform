use super::*;

/// Memoized type/template graph. Reference values are leaves, not owning edges.
struct Demands<'a, 'b> {
    tables: &'a ValidationContext<'b>,
    layout: &'b StorageLayout,
    members: Vec<(usize, usize)>,
    values: Vec<Option<Demand>>,
    active: Vec<bool>,
}

/// Cache the full suffix height as well as node count. A shallow prior visit
/// must not make that suffix invisible to a later, deeper declaration path.
#[derive(Clone, Copy)]
struct Demand {
    nodes: u32,
    height: u8,
}

impl Demand {
    const LEAF: Self = Self {
        nodes: 1,
        height: 0,
    };

    fn through(self) -> Result<Self, BytecodeError> {
        Ok(Self {
            nodes: self.nodes,
            height: self
                .height
                .checked_add(1)
                .ok_or(RejectionReason::ConstructionDemandOverflow)?,
        })
    }

    fn include(&mut self, child: Self, count: u32) -> Result<(), BytecodeError> {
        self.height = self.height.max(child.through()?.height);
        self.nodes = add(
            self.nodes,
            child
                .nodes
                .checked_mul(count)
                .ok_or(RejectionReason::ConstructionDemandOverflow)?,
        )?;
        Ok(())
    }

    fn check_depth(self, depth: u8) -> Result<Self, BytecodeError> {
        if usize::from(depth) + usize::from(self.height)
            > crate::bytecode::BYTECODE_MAX_TYPE_NESTING
        {
            return Err(RejectionReason::ConstructionDemandOverflow.into());
        }
        Ok(self)
    }
}

impl<'a, 'b> Demands<'a, 'b> {
    fn new(
        tables: &'a ValidationContext<'b>,
        layout: &'b StorageLayout,
        budget: &mut ValidationBudget,
    ) -> Result<Self, BytecodeError> {
        let count = tables
            .types
            .entries
            .len()
            .checked_add(tables.index.entries.len())
            .ok_or(RejectionReason::ConstructionDemandOverflow)?;
        let mut values = Vec::new();
        let mut active = Vec::new();
        budget.reserve(&mut values, count)?;
        budget.reserve(&mut active, count)?;
        budget.work(count)?;
        values.resize(count, None);
        active.resize(count, false);
        let mut members = Vec::new();
        for (position, declaration) in layout.entries.iter().enumerate() {
            budget.work(1)?;
            if declaration.owner != StorageOwner::Instance
                || declaration.role == StorageRole::External
            {
                continue;
            }
            let owner = declaration
                .owner_pou_id
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            let pou = tables
                .pou(owner, budget)?
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            let template = if pou.kind == PouKind::Method {
                pou.owner_pou_id
                    .ok_or(RejectionReason::InvalidConstructionRecord)?
            } else {
                owner
            };
            let position_in_index = tables
                .pou_position(template, budget)?
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            budget.push(&mut members, (position_in_index, position))?;
        }
        budget.sort_by(&mut members, &mut |a, b, _| Ok(a.cmp(b)))?;
        Ok(Self {
            tables,
            layout,
            members,
            values,
            active,
        })
    }

    fn node(
        &mut self,
        node: usize,
        depth: u8,
        budget: &mut ValidationBudget,
    ) -> Result<Demand, BytecodeError> {
        budget.work(1)?;
        if usize::from(depth) > crate::bytecode::BYTECODE_MAX_TYPE_NESTING
            || node >= self.values.len()
            || self.active[node]
        {
            return Err(RejectionReason::ConstructionDemandOverflow.into());
        }
        if let Some(value) = self.values[node] {
            return value.check_depth(depth);
        }
        self.active[node] = true;
        let tables = self.tables;
        let type_count = tables.types.entries.len();
        let value = if node < type_count {
            match &tables.types.entries[node].data {
                TypeData::Alias { target_type_id } => self
                    .node(*target_type_id as usize, depth + 1, budget)?
                    .through()?,
                TypeData::Array { elem_type_id, dims } => {
                    let mut count = 1u32;
                    for &(low, high) in dims {
                        budget.work(1)?;
                        if high == i64::MAX {
                            count = 0;
                            break;
                        }
                        let length = high
                            .checked_sub(low)
                            .and_then(|v| v.checked_add(1))
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or(RejectionReason::ConstructionDemandOverflow)?;
                        count = count
                            .checked_mul(length)
                            .ok_or(RejectionReason::ConstructionDemandOverflow)?;
                    }
                    let element = self.node(*elem_type_id as usize, depth + 1, budget)?;
                    let mut demand = Demand::LEAF;
                    demand.include(element, count)?;
                    demand
                }
                TypeData::Struct { fields } | TypeData::Union { fields } => {
                    let mut total = Demand::LEAF;
                    for field in fields {
                        total.include(self.node(field.type_id as usize, depth + 1, budget)?, 1)?;
                    }
                    total
                }
                TypeData::Pou { pou_id } => {
                    let at = self
                        .tables
                        .pou_position(*pou_id, budget)?
                        .ok_or(RejectionReason::InvalidConstructionRecord)?;
                    self.node(type_count + at, depth + 1, budget)?.through()?
                }
                TypeData::Enum { base_type_id, .. } | TypeData::Subrange { base_type_id, .. } => {
                    let base = self.node(*base_type_id as usize, depth + 1, budget)?;
                    Demand {
                        nodes: 1,
                        height: base.through()?.height,
                    }
                }
                _ => Demand::LEAF,
            }
        } else {
            let position = node - type_count;
            let pou = &self.tables.index.entries[position];
            let mut total = Demand::LEAF;
            if let Some(parent) = pou.class_meta.as_ref().and_then(|meta| meta.parent_pou_id) {
                let at = self
                    .tables
                    .pou_position(parent, budget)?
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                total.include(self.node(type_count + at, depth + 1, budget)?, 1)?;
            }
            let start = budget.lower_bound(&self.members, |v, _| Ok(v.0 < position))?;
            let end = budget.lower_bound(&self.members, |v, _| Ok(v.0 <= position))?;
            for at in start..end {
                let declaration = &self.layout.entries[self.members[at].1];
                let ty = declaration
                    .type_id
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                total.include(self.node(ty as usize, depth + 1, budget)?, 1)?;
            }
            total
        };
        if value.nodes > BYTECODE_MAX_CONSTRUCTION_NODES {
            return Err(RejectionReason::ConstructionDemandOverflow.into());
        }
        let value = value.check_depth(depth)?;
        self.active[node] = false;
        self.values[node] = Some(value);
        Ok(value)
    }
}

fn add(a: u32, b: u32) -> Result<u32, BytecodeError> {
    a.checked_add(b)
        .filter(|v| *v <= BYTECODE_MAX_CONSTRUCTION_NODES)
        .ok_or_else(|| RejectionReason::ConstructionDemandOverflow.into())
}

pub(super) fn declaration_demands(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    budget: &mut ValidationBudget,
) -> Result<Vec<u32>, BytecodeError> {
    let mut graph = Demands::new(tables, layout, budget)?;
    let mut values = Vec::new();
    budget.reserve(&mut values, layout.entries.len())?;
    for declaration in &layout.entries {
        let value = match declaration.role {
            StorageRole::External => 0,
            StorageRole::Scratch => {
                let pou = tables
                    .pou(
                        declaration
                            .owner_pou_id
                            .ok_or(RejectionReason::InvalidConstructionRecord)?,
                        budget,
                    )?
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                pou.local_ref_count
                    .checked_sub(declaration.slot)
                    .ok_or(RejectionReason::InvalidConstructionRecord)?
            }
            StorageRole::ProgramRoot => {
                let position = tables
                    .pou_position(
                        declaration
                            .owner_pou_id
                            .ok_or(RejectionReason::InvalidConstructionRecord)?,
                        budget,
                    )?
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                graph
                    .node(tables.types.entries.len() + position, 0, budget)?
                    .nodes
            }
            _ => {
                graph
                    .node(
                        declaration
                            .type_id
                            .ok_or(RejectionReason::InvalidConstructionRecord)?
                            as usize,
                        0,
                        budget,
                    )?
                    .nodes
            }
        };
        values.push(value);
    }
    Ok(values)
}
