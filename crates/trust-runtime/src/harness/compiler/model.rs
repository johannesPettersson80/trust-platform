use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::program_model::Expr;
use crate::program_model::VarDef;
use crate::task::ProgramDef;
use crate::value::DateTimeProfile;
use trust_hir::TypeId;

pub(crate) struct LoweredProgram {
    pub(crate) program: ProgramDef,
    pub(crate) globals: Vec<GlobalInit>,
}

pub(crate) struct ProgramVars {
    pub(crate) globals: Vec<GlobalInit>,
    pub(crate) vars: Vec<VarDef>,
    pub(crate) temps: Vec<VarDef>,
}

pub(crate) struct ConfigModel {
    pub(crate) globals: Vec<GlobalInit>,
    pub(crate) tasks: Vec<crate::task::TaskConfig>,
    pub(crate) programs: Vec<ProgramInstanceConfig>,
    pub(crate) using: Vec<SmolStr>,
    pub(crate) access: Vec<AccessDecl>,
    pub(crate) config_inits: Vec<ConfigInit>,
}

pub(crate) struct ProgramInstanceConfig {
    pub(crate) name: SmolStr,
    pub(crate) type_name: SmolStr,
    pub(crate) task: Option<SmolStr>,
    pub(crate) retain: Option<crate::RetainPolicy>,
    pub(crate) fb_tasks: Vec<FbTaskBinding>,
}

#[derive(Debug, Clone)]
pub(crate) struct FbTaskBinding {
    pub(crate) path: AccessPath,
    pub(crate) task: SmolStr,
}

#[derive(Debug, Clone)]
pub(crate) enum AccessPath {
    Direct { address: IoAddress, text: SmolStr },
    Parts(Vec<AccessPart>),
}

#[derive(Debug, Clone)]
pub(crate) enum AccessPart {
    Name(SmolStr),
    Index(Vec<i64>),
    Partial(crate::value::PartialAccess),
}

#[derive(Debug, Clone)]
pub(crate) struct AccessDecl {
    pub(crate) name: SmolStr,
    pub(crate) path: AccessPath,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigInit {
    pub(crate) path: AccessPath,
    pub(crate) address: Option<IoAddress>,
    pub(crate) type_id: TypeId,
    pub(crate) initializer: Option<Expr>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedAccess {
    Direct(IoAddress),
    Variable {
        reference: crate::value::ValueRef,
        partial: Option<crate::value::PartialAccess>,
    },
}

#[derive(Clone)]
pub(crate) struct GlobalInit {
    pub(crate) name: SmolStr,
    pub(crate) type_id: TypeId,
    pub(crate) initializer: Option<Expr>,
    pub(crate) retain: crate::RetainPolicy,
    pub(crate) address: Option<SmolStr>,
}

#[derive(Clone)]
pub(crate) struct WildcardRequirement {
    pub(crate) name: SmolStr,
    pub(crate) reference: crate::value::ValueRef,
    pub(crate) area: IoArea,
}

pub(crate) struct LoweringContext<'a> {
    pub(crate) registry: &'a mut trust_hir::types::TypeRegistry,
    pub(crate) profile: DateTimeProfile,
    pub(crate) using: Vec<SmolStr>,
    pub(crate) file_id: u32,
    pub(crate) statement_locations: &'a mut Vec<SourceLocation>,
}
