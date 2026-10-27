//! Where every variable of a compiled program lies (specification 44 §7): the globals, the program
//! instances, their areas, the tasks that run them and the variables bound to the process image.
//!
//! The non-retained globals lie in `G`, each program instance's non-retained variables in `P`, and
//! every retained variable, global or of a program, in `R` (§7.1): a program type is laid out as two
//! structures, its variables and its retained variables, and each instance has one of each.

use smol_str::SmolStr;
use trust_hir::TypeId;

use super::types::{lay_out, mangle, Kind, Member, Ty, Types};
use super::NativeError;
use crate::harness::{AccessPart, AccessPath, ConfigModel, ConnectionEnd, LoweredApplication};
use crate::io::{IoAddress, IoSize};
use crate::task::ProgramDef;
use crate::RetainPolicy;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::{SYMBOL_CONSTANT, SYMBOL_INSTANCE, SYMBOL_RETAINED};

/// Whether a declaration's qualifier keeps the variable across a warm restart.
pub(super) fn retained(policy: RetainPolicy) -> bool {
    matches!(policy, RetainPolicy::Retain | RetainPolicy::Persistent)
}

/// A global variable.
pub(super) struct Global {
    pub(super) name: SmolStr,
    pub(super) ty: Ty,
    pub(super) retained: bool,
    pub(super) constant: bool,
    /// The process-image address the global is declared `AT`, when it is directly addressed.
    pub(super) address: Option<IoAddress>,
}

/// A program type: its two structures, the variables and the retained variables.
pub(super) struct ProgramType<'a> {
    pub(super) def: &'a ProgramDef,
    /// The Rust type of the variables, `P_<NAME>`.
    pub(super) rust: String,
    /// The Rust type of the retained variables, `PR_<NAME>`.
    pub(super) rust_retain: String,
    /// The Rust function of its body, `prog_<NAME>`.
    pub(super) body: String,
    /// Whether its instances are declared `RETAIN` in the configuration, which retains each of
    /// its variables declared without a retention qualifier that is no instance (specification 11
    /// §6.7): such instances have a type of their own, laid out apart.
    pub(super) retain_all: bool,
    pub(super) members: Vec<Member>,
    pub(super) size: u32,
    pub(super) align: u32,
    pub(super) retained: Vec<Member>,
    pub(super) retain_size: u32,
    pub(super) retain_align: u32,
}

impl ProgramInstance {
    /// Whether the configuration connects an output (`output`) or an input of the instance to a
    /// global or a constant, which its entry makes around its body.
    pub(super) fn connects(&self, output: bool) -> bool {
        self.connections.iter().any(|connection| {
            connection.output == output && !matches!(connection.end, ConnectionEnd::Address { .. })
        })
    }
}

impl ProgramType<'_> {
    /// The member named `name`, compared without case, and whether it is retained.
    pub(super) fn member(&self, name: &str) -> Option<(&Member, bool)> {
        self.members
            .iter()
            .map(|member| (member, false))
            .chain(self.retained.iter().map(|member| (member, true)))
            .find(|(member, _)| member.name.eq_ignore_ascii_case(name))
    }
}

/// A program instance of the configuration.
pub(super) struct ProgramInstance {
    pub(super) name: SmolStr,
    /// The index of its type in [`Model::program_types`].
    pub(super) program: usize,
    /// The connections of its inputs and outputs (IEC §6.8.2, Table 62 8a, 8b, 9a, 9b): one to a
    /// directly represented variable is a binding of the input or output there (`bindings`), the
    /// others its entry makes around the body (`emit::connections`).
    pub(super) connections: Vec<crate::harness::ProgramConnection>,
}

/// A task with the program instances it runs, in their order.
pub(super) struct Task {
    pub(super) name: SmolStr,
    pub(super) interval_ns: u64,
    pub(super) priority: u32,
    pub(super) single: Option<SmolStr>,
    pub(super) programs: Vec<usize>,
    /// The function block instances the task runs on their own, after its programs, as indexes
    /// into [`Model::block_tasks`] (IEC §6.8.2, `fb WITH task`).
    pub(super) blocks: Vec<usize>,
}

/// A function block instance of a program instance that a task runs on its own (IEC §6.8.2).
pub(super) struct BlockTask {
    /// The program instance, an index into [`Model::instances`].
    pub(super) instance: usize,
    /// The path from the program instance to the function block instance.
    pub(super) path: Vec<SmolStr>,
    pub(super) task: SmolStr,
}

/// A variable bound to a process image address (`AT %…` or `VAR_CONFIG`), or one leaf of a located
/// array or structure (`located`).
pub(super) struct Binding {
    /// The path of the variable's symbol: a global's name, or the program instance first.
    pub(super) path: Vec<SmolStr>,
    pub(super) address: IoAddress,
    /// The offset of the bound leaf within the variable.
    pub(super) leaf: u32,
    /// The leaf's codec: `BIND_RAW` or `BIND_TIME_MILLIS`.
    pub(super) codec: u32,
}

/// One field of an area: a global, or a program instance's part.
pub(super) struct AreaField {
    /// The name as declared.
    pub(super) name: SmolStr,
    /// The Rust type.
    pub(super) rust: String,
    pub(super) offset: u32,
}

/// The fields of one area and its size.
#[derive(Default)]
pub(super) struct AreaLayout {
    pub(super) fields: Vec<AreaField>,
    pub(super) size: u32,
}

impl AreaLayout {
    pub(super) fn new(parts: Vec<(SmolStr, String, u32, u32)>) -> Self {
        let mut offset = 0u32;
        let mut align = 1u32;
        let mut fields = Vec::with_capacity(parts.len());
        for (name, rust, size, part_align) in parts {
            offset = offset.div_ceil(part_align) * part_align;
            fields.push(AreaField { name, rust, offset });
            offset += size;
            align = align.max(part_align);
        }
        Self {
            fields,
            size: offset.div_ceil(align) * align,
        }
    }

    /// The field named `name`, compared without case.
    pub(super) fn field(&self, name: &str) -> Option<&AreaField> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }
}

/// Where everything lies.
pub(super) struct Model<'a> {
    pub(super) types: Types<'a>,
    /// The runtime's construction of the program: its storage, types and initializers, from which
    /// the build computes starting values (specification 44 §6.7).
    pub(super) builder: &'a crate::RuntimeBuilder,
    /// The project's functions.
    pub(super) functions: &'a indexmap::IndexMap<SmolStr, crate::program_model::FunctionDef>,
    pub(super) globals: Vec<Global>,
    pub(super) program_types: Vec<ProgramType<'a>>,
    pub(super) instances: Vec<ProgramInstance>,
    pub(super) tasks: Vec<Task>,
    /// The function block instances tasks run on their own (IEC §6.8.2).
    pub(super) block_tasks: Vec<BlockTask>,
    pub(super) bindings: Vec<Binding>,
    /// The non-retained globals.
    pub(super) area_g: AreaLayout,
    /// The retained globals and the program instances' retained variables.
    pub(super) area_r: AreaLayout,
    /// The program instances' variables.
    pub(super) area_p: AreaLayout,
    /// The bytes of the input, output and marker images that the bindings reach.
    pub(super) images: [u32; 3],
    /// The bytes of the stack area: the largest pair of texts that a failed assertion lays there
    /// for the kernel (specification 44 §7.1, Appendix A), 0 without one.
    pub(super) stack: u32,
    /// Whether a body reads the platform's wall clock, `CURRENT_DT` (specification 44 §5.2).
    pub(super) wall_clock: bool,
    /// The conversion rules the program is built with, its project's `vendor_profile`
    /// (specification 07 §2.10); the image's header records them (specification 44 §5.2).
    pub(super) conversions: crate::stdlib::conversions::ConversionProfile,
    /// The access paths the configuration declares (`VAR_ACCESS`, IEC §6.5.2.2), which a body may
    /// name as the variables they name (specification 03 §2).
    pub(super) access: Vec<crate::harness::AccessDecl>,
    /// The access paths the programs declare, each path starting with its program's type: each
    /// instance has its own, which the program's body names (IEC §6.5.2.2, Table 47).
    pub(super) program_access: Vec<crate::harness::AccessDecl>,
}

impl<'a> Model<'a> {
    /// Lays out the program of `lowered`.
    ///
    /// # Errors
    ///
    /// [`NativeError::Unsupported`] for a declaration the generator does not compile yet.
    pub(super) fn new(
        lowered: &'a LoweredApplication,
        registry: trust_hir::types::TypeRegistry,
        tests: &[SmolStr],
        direct: &[IoAddress],
    ) -> Result<Self, NativeError> {
        let builder = &lowered.builder;
        let mut types = Types::new(
            registry,
            super::classes::Defs::new(
                builder.function_blocks(),
                builder.classes(),
                builder.interfaces(),
                builder.edge_inputs(),
            ),
        );
        let Some(configuration) = lowered.configuration.as_ref() else {
            return Err(NativeError::Compile(
                "the project has no CONFIGURATION with a RESOURCE to compile".into(),
            ));
        };
        let globals = globals(lowered, configuration, direct, &mut types)?;
        let (program_types, instances, block_tasks) =
            instances(lowered, configuration, &mut types)?;
        let tasks = tasks(configuration, tests, &block_tasks);
        let mut declared = bindings(configuration, &globals, &instances, &program_types)?;
        declared.extend(super::located::block_members(
            &types,
            &instances,
            &program_types,
        ));
        let bindings =
            super::located::bindings(&types, declared, &globals, &instances, &program_types)?;
        let images = images(&bindings)?;
        let area_g = AreaLayout::new(
            globals
                .iter()
                .filter(|global| !global.retained)
                .map(|global| area_part(&types, global.name.clone(), global.ty))
                .collect(),
        );
        let mut retained_parts: Vec<_> = globals
            .iter()
            .filter(|global| global.retained)
            .map(|global| area_part(&types, global.name.clone(), global.ty))
            .collect();
        retained_parts.extend(instances.iter().map(|instance| {
            let program = &program_types[instance.program];
            (
                instance.name.clone(),
                program.rust_retain.clone(),
                program.retain_size,
                program.retain_align,
            )
        }));
        let area_r = AreaLayout::new(retained_parts);
        let area_p = AreaLayout::new(
            instances
                .iter()
                .map(|instance| {
                    let program = &program_types[instance.program];
                    (
                        instance.name.clone(),
                        program.rust.clone(),
                        program.size,
                        program.align,
                    )
                })
                .collect(),
        );
        Ok(Self {
            types,
            builder,
            functions: builder.functions(),
            globals,
            program_types,
            instances,
            tasks,
            block_tasks,
            bindings,
            area_g,
            area_r,
            area_p,
            images,
            stack: 0,
            wall_clock: false,
            conversions: crate::stdlib::conversions::ConversionProfile::Iec,
            access: configuration.access.clone(),
            program_access: lowered.program_access.clone(),
        })
    }

    /// The global named `name`, compared without case.
    pub(super) fn global(&self, name: &str) -> Option<&Global> {
        self.globals
            .iter()
            .find(|global| global.name.eq_ignore_ascii_case(name))
    }

    /// The function named `name`: its qualified name, or its own name when that is unique, both
    /// compared without case.
    pub(super) fn function(&self, name: &str) -> Option<&'a crate::program_model::FunctionDef> {
        let functions = self.functions;
        functions
            .values()
            .find(|def| def.name.eq_ignore_ascii_case(name))
            .or_else(|| {
                let mut matching = functions.values().filter(|def| {
                    def.name
                        .rsplit('.')
                        .next()
                        .is_some_and(|last| last.eq_ignore_ascii_case(name))
                });
                let first = matching.next()?;
                matching.next().is_none().then_some(first)
            })
    }

    /// The type of registry type `id`.
    pub(super) fn ty(&mut self, id: TypeId) -> Result<Ty, NativeError> {
        self.types.of(id)
    }
}

fn area_part(types: &Types<'_>, name: SmolStr, ty: Ty) -> (SmolStr, String, u32, u32) {
    let info = types.info(ty);
    (name, info.rust.clone(), info.size, info.align)
}

/// The layout of a program type: its variables and its retained variables, `VAR_EXTERNAL`
/// excluded (they name globals) and `VAR_TEMP` excluded (they live on the stack).
fn program_type<'a>(
    def: &'a ProgramDef,
    types: &mut Types<'_>,
    retain_all: bool,
) -> Result<ProgramType<'a>, NativeError> {
    let mut plain = Vec::new();
    let mut kept = Vec::new();
    let edges = types.defs.edge_inputs(&def.name);
    for var in &def.vars {
        if var.external {
            continue;
        }
        if var.in_out {
            return Err(NativeError::unsupported(format!(
                "VAR_IN_OUT {} of program {}",
                var.name, def.name
            )));
        }
        let ty = types.of(var.type_id)?;
        let mut flags = if var.constant { SYMBOL_CONSTANT } else { 0 };
        if matches!(types.info(ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
            flags |= SYMBOL_INSTANCE;
        }
        // An instance follows its own type's declarations, whatever the program instance's
        // qualifier (specification 11 §6.7).
        let qualified =
            retain_all && var.retain == RetainPolicy::Unspecified && flags & SYMBOL_INSTANCE == 0;
        // An edge-qualified input's trigger lies beside it, retained as it is (IEC §6.5.2.2).
        let trigger = edges
            .iter()
            .find(|edge| edge.name.eq_ignore_ascii_case(&var.name))
            .map(|edge| {
                (
                    super::types::edge_trigger_name(&var.name),
                    types.edge_trigger(edge),
                )
            });
        if retained(var.retain) || qualified {
            kept.push((var.name.clone(), ty, flags | SYMBOL_RETAINED));
            kept.extend(trigger.map(|(name, ty)| (name, ty, SYMBOL_RETAINED)));
        } else {
            plain.push((var.name.clone(), ty, flags));
            plain.extend(trigger.map(|(name, ty)| (name, ty, 0)));
        }
    }
    let (members, size, align) = lay_out(plain, types);
    let (retained, retain_size, retain_align) = lay_out(kept, types);
    let name = if retain_all {
        format!("{}_RETAIN", mangle(&def.name))
    } else {
        mangle(&def.name)
    };
    Ok(ProgramType {
        def,
        rust: format!("P_{name}"),
        rust_retain: format!("PR_{name}"),
        body: format!("prog_{name}"),
        retain_all,
        members,
        size,
        align,
        retained,
        retain_size,
        retain_align,
    })
}

/// The globals of the files and of the configuration, each once.
fn globals(
    lowered: &LoweredApplication,
    configuration: &ConfigModel,
    direct: &[IoAddress],
    types: &mut Types<'_>,
) -> Result<Vec<Global>, NativeError> {
    let mut globals: Vec<Global> = Vec::new();
    for global in lowered.globals.iter().chain(&configuration.globals) {
        if globals
            .iter()
            .any(|known| known.name.eq_ignore_ascii_case(&global.name))
        {
            continue;
        }
        let address = global
            .address
            .as_deref()
            .map(|text| {
                IoAddress::parse(text).map_err(|error| {
                    NativeError::Compile(format!(
                        "the address {text} of global {}: {error}",
                        global.name
                    ))
                })
            })
            .transpose()?;
        globals.push(Global {
            name: global.name.clone(),
            ty: types.of(global.type_id)?,
            retained: retained(global.retain),
            constant: global.constant,
            address,
        });
    }
    // A directly represented variable that a body names (IEC §6.5.5.1, Table 16) is a global of its
    // address's own name, `%IX0.1`, which no declaration can have, at its address as a global
    // declared `AT` it is: the scan samples it at the input sample and publishes it at the output
    // sample (specification 44 §5.6), its type the size's (`%IW2` a `WORD`).
    for address in direct {
        let type_id = match address.size {
            IoSize::Bit => TypeId::BOOL,
            IoSize::Byte => TypeId::BYTE,
            IoSize::Word => TypeId::WORD,
            IoSize::DWord => TypeId::DWORD,
            IoSize::LWord | IoSize::Bytes(_) => TypeId::LWORD,
        };
        globals.push(Global {
            name: SmolStr::new(address.to_string()),
            ty: types.of(type_id)?,
            retained: false,
            constant: false,
            address: Some(address.clone()),
        });
    }
    // A function's `VAR_STAT` variables live as long as the program, one of each per function
    // (specification 01 §3): globals of hidden names, which no source names.
    for function in lowered.builder.functions().values() {
        for local in &function.static_locals {
            globals.push(Global {
                name: crate::program_model::static_storage_name(&function.name, &local.name),
                ty: types.of(local.type_id)?,
                retained: retained(local.retain),
                constant: local.constant,
                address: None,
            });
        }
    }
    Ok(globals)
}

/// The program instances of the configuration and the layout of each program type they use.
#[allow(clippy::type_complexity)]
fn instances<'a>(
    lowered: &'a LoweredApplication,
    configuration: &ConfigModel,
    types: &mut Types<'_>,
) -> Result<(Vec<ProgramType<'a>>, Vec<ProgramInstance>, Vec<BlockTask>), NativeError> {
    let mut program_types: Vec<ProgramType<'a>> = Vec::new();
    let mut instances = Vec::new();
    let mut block_tasks = Vec::new();
    for config in &configuration.programs {
        let Some(def) = lowered
            .program_defs
            .values()
            .find(|def| def.name.eq_ignore_ascii_case(&config.type_name))
        else {
            return Err(NativeError::Compile(format!(
                "program {} of instance {} is not declared",
                config.type_name, config.name
            )));
        };
        let retain_all = config.retain.is_some_and(retained);
        for binding in &config.fb_tasks {
            let AccessPath::Parts(parts) = &binding.path else {
                return Err(NativeError::unsupported(format!(
                    "a function block of {} bound to a task by an address",
                    config.name
                )));
            };
            let mut path = Vec::with_capacity(parts.len());
            for part in parts {
                let AccessPart::Name(name) = part else {
                    return Err(NativeError::unsupported(format!(
                        "a function block of {} bound to a task through an index",
                        config.name
                    )));
                };
                path.push(name.clone());
            }
            block_tasks.push(BlockTask {
                instance: instances.len(),
                path,
                task: binding.task.clone(),
            });
        }
        let program = if let Some(index) = program_types
            .iter()
            .position(|known| std::ptr::eq(known.def, def) && known.retain_all == retain_all)
        {
            index
        } else {
            program_types.push(program_type(def, types, retain_all)?);
            program_types.len() - 1
        };
        instances.push(ProgramInstance {
            name: config.name.clone(),
            program,
            connections: config.connections.clone(),
        });
    }
    Ok((program_types, instances, block_tasks))
}

/// The tasks with their program instances, and the programs without a task as the task of the
/// lowest priority (specification 10 §4). The test programs `tests` are instances that no task runs,
/// the background task included: only a test's own invocation runs one (specification 22), so a
/// device that starts a test image runs none of them by itself.
fn tasks(configuration: &ConfigModel, tests: &[SmolStr], block_tasks: &[BlockTask]) -> Vec<Task> {
    let mut tasks = Vec::new();
    for task in &configuration.tasks {
        let interval = u64::try_from(task.interval.as_nanos()).unwrap_or(0);
        let programs = configuration
            .programs
            .iter()
            .enumerate()
            .filter(|(_, config)| {
                config
                    .task
                    .as_ref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(&task.name))
            })
            .map(|(index, _)| index)
            .collect();
        let blocks = block_tasks
            .iter()
            .enumerate()
            .filter(|(_, block)| block.task.eq_ignore_ascii_case(&task.name))
            .map(|(index, _)| index)
            .collect();
        tasks.push(Task {
            name: task.name.clone(),
            interval_ns: interval,
            priority: task.priority,
            single: task.single.clone(),
            programs,
            blocks,
        });
    }
    let background: Vec<usize> = configuration
        .programs
        .iter()
        .enumerate()
        .filter(|(_, config)| {
            config.task.is_none()
                && !tests
                    .iter()
                    .any(|test| test.eq_ignore_ascii_case(&config.name))
        })
        .map(|(index, _)| index)
        .collect();
    if !background.is_empty() {
        tasks.push(Task {
            name: SmolStr::new_static(""),
            interval_ns: 0,
            priority: u32::MAX,
            single: None,
            programs: background,
            blocks: Vec::new(),
        });
    }
    tasks
}

/// The variables bound to the process image: the directly addressed globals, `VAR_CONFIG` and `AT`
/// in the programs.
#[allow(clippy::type_complexity)]
fn bindings(
    configuration: &ConfigModel,
    globals: &[Global],
    instances: &[ProgramInstance],
    program_types: &[ProgramType<'_>],
) -> Result<Vec<(Vec<SmolStr>, IoAddress)>, NativeError> {
    // A variable at an incomplete location (`AT %I*`) takes the address the configuration's
    // `VAR_CONFIG` gives it (IEC §6.5.5.4), and a build without one fails.
    let mut incomplete: Vec<(Vec<SmolStr>, IoArea)> = Vec::new();
    let mut bindings: Vec<(Vec<SmolStr>, IoAddress)> = Vec::new();
    for global in globals {
        let Some(address) = global.address.clone() else {
            continue;
        };
        let path = vec![global.name.clone()];
        if address.wildcard {
            incomplete.push((path, address.area));
        } else {
            bindings.push((path, address));
        }
    }
    for init in &configuration.config_inits {
        let Some(address) = init.address.clone() else {
            continue;
        };
        let AccessPath::Parts(parts) = &init.path else {
            return Err(NativeError::unsupported("a VAR_CONFIG direct path".into()));
        };
        let mut path = Vec::with_capacity(parts.len());
        for part in parts {
            let AccessPart::Name(name) = part else {
                return Err(NativeError::unsupported(
                    "a VAR_CONFIG path with an index or a partial access".into(),
                ));
            };
            path.push(name.clone());
        }
        bindings.push((path, address));
    }
    for instance in instances {
        let program = &program_types[instance.program];
        for var in &program.def.vars {
            if let Some(address) = var.address.clone() {
                let path = vec![instance.name.clone(), var.name.clone()];
                if address.wildcard {
                    incomplete.push((path, address.area));
                } else {
                    bindings.push((path, address));
                }
            }
        }
    }
    // An input or an output that the configuration connects to a directly represented variable
    // lies there for its instance, as `VAR_CONFIG` locates a variable: the scan samples the input
    // and publishes the output (IEC §6.8.2, Table 62 8a, 9a). An input reads an input or a marker
    // address, an output writes an output or a marker address.
    for instance in instances {
        let program = &program_types[instance.program];
        for connection in &instance.connections {
            let ConnectionEnd::Address { address, text } = &connection.end else {
                continue;
            };
            let named = || format!("{}.{}", instance.name, connection.parameter);
            let Some(var) = program
                .def
                .vars
                .iter()
                .find(|var| var.name.eq_ignore_ascii_case(&connection.parameter))
            else {
                return Err(NativeError::Compile(format!(
                    "the program {} of instance {} has no input or output {}",
                    program.def.name, instance.name, connection.parameter
                )));
            };
            if var.address.is_some() {
                return Err(NativeError::Compile(format!(
                    "{} is connected to {text} and declared at an address",
                    named()
                )));
            }
            let (allowed, what) = if connection.output {
                (
                    IoArea::Output,
                    "an output writes an output or a marker address",
                )
            } else {
                (IoArea::Input, "an input reads an input or a marker address")
            };
            if address.area != allowed && address.area != IoArea::Memory {
                return Err(NativeError::Compile(format!(
                    "{} is connected to {text}: {what} (IEC 61131-3 Table 62 8a, 9a)",
                    named()
                )));
            }
            let path = vec![instance.name.clone(), var.name.clone()];
            if bindings.iter().any(|(bound, _)| same_path(bound, &path)) {
                return Err(NativeError::Compile(format!(
                    "{} is connected to {text} and located by VAR_CONFIG",
                    named()
                )));
            }
            bindings.push((path, address.clone()));
        }
    }
    // A location entry that names a variable by its name alone, no global, completes the one
    // incomplete location of that name (specification 03 §2): `out AT %QX0.1` gives `P1.out` its
    // address when no other program instance declares an `out` at an incomplete location.
    for (bound, _) in &mut bindings {
        if let [name] = bound.as_slice()
            && !globals
                .iter()
                .any(|global| global.name.eq_ignore_ascii_case(name))
        {
            let mut named = incomplete.iter().map(|(path, _)| path).filter(|path| {
                path.last()
                    .is_some_and(|last| last.eq_ignore_ascii_case(name))
            });
            match (named.next(), named.next()) {
                (Some(path), None) => *bound = path.clone(),
                (Some(_), Some(_)) => {
                    return Err(NativeError::Compile(format!(
                        "VAR_CONFIG {name} names an incomplete location of several program \
                         instances"
                    )));
                }
                _ => {}
            }
        }
    }
    // The asterisk stands for the size and the address only: the completion keeps the declared
    // area (IEC §6.5.5.4; specification 03 §5).
    for (path, area) in incomplete {
        let configured = bindings.iter().find(|(bound, _)| {
            bound.len() == path.len()
                && bound
                    .iter()
                    .zip(&path)
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
        });
        match configured {
            None => {
                return Err(NativeError::Compile(format!(
                    "{} lies at an incomplete location that no VAR_CONFIG completes",
                    path.join(".")
                )));
            }
            Some((_, address)) if address.area != area => {
                return Err(NativeError::Compile(format!(
                    "VAR_CONFIG completes {}, declared at {}*, with an address of another area",
                    path.join("."),
                    area_prefix(area)
                )));
            }
            Some(_) => {}
        }
    }
    Ok(bindings)
}

/// Whether two symbol paths are the same, compared without case.
fn same_path(a: &[SmolStr], b: &[SmolStr]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// The prefix of the process image area `area`: `%I`, `%Q` or `%M`.
fn area_prefix(area: IoArea) -> &'static str {
    match area {
        IoArea::Input => "%I",
        IoArea::Output => "%Q",
        IoArea::Memory => "%M",
    }
}

/// The bytes of the input, output and marker images the bindings reach.
fn images(bindings: &[Binding]) -> Result<[u32; 3], NativeError> {
    let mut images = [0u32; 3];
    for binding in bindings {
        // A hierarchical address lies in the kernel's hierarchical image (§5.6).
        if binding.address.path.len() > 1 {
            continue;
        }
        let Ok(Some(end)) = binding.address.flat_byte_end() else {
            return Err(NativeError::unsupported(format!(
                "the address of {}",
                binding.path.join(".")
            )));
        };
        let (image, area) = match binding.address.area {
            trust_runtime_core::memory::IoArea::Input => (0, "%I"),
            trust_runtime_core::memory::IoArea::Output => (1, "%Q"),
            trust_runtime_core::memory::IoArea::Memory => (2, "%M"),
        };
        // Each process image holds at most 16 MiB (specification 11 §6.4).
        let len = usize::try_from(end).unwrap_or(usize::MAX);
        trust_runtime_core::io_image::validate_process_image_area_len(
            &format!("the {area} image of {}", binding.path.join(".")),
            len,
        )
        .map_err(|error| NativeError::Compile(error.to_string()))?;
        images[image] = images[image].max(end);
    }
    Ok(images)
}
