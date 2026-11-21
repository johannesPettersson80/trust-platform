//! The bench of a project's Rust POUs (specification 48 RU-48): the test programs that call each
//! case of each Rust function, function block and method, which `trust-dev rust bench` builds into
//! a debug image with the program and runs once a scan, and the cases of each Rust program, whose
//! inputs it writes into the program's configured instances before each scan of them.

use std::fmt::Write as _;

use smol_str::SmolStr;
use trust_plc_decl::{Access, Case, DataType, Kind, Section, Value, Variable};

use super::RustCrate;
use crate::native::{HostCode, NativeProgram};

/// One generated test program: the Rust POU it measures and its case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchProgram {
    /// The test program's name, which the build instantiates.
    pub program: SmolStr,
    /// The POU's name in the image's Rust POU table and in `[timing.measured]`: `FILTER_EMA`, a
    /// method's `AXIS.JOG`.
    pub pou: String,
    /// The case as it gives its values, for the command's report: `x = 1.0E6, alpha = 0.001`;
    /// empty for the declared initial values.
    pub case: String,
}

/// One case of a Rust program: the inputs the bench writes into each instance of the program that
/// the project's configuration declares before each scan of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramCase {
    /// The program's name, which its instances name as their type, and its name in the image's
    /// Rust POU table: `CELL`.
    pub pou: String,
    /// The inputs the case gives, each by its name with its value as a Structured Text literal of
    /// its type.
    pub inputs: Vec<(String, String)>,
    /// The case as it gives its values, for the command's report; empty for the declared initial
    /// values.
    pub case: String,
}

/// The test programs of a bench, with their Structured Text, and the cases of its Rust programs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bench {
    /// The test programs' source, one file.
    pub source: String,
    /// The test programs, in the order of the crate's POUs and their cases.
    pub programs: Vec<BenchProgram>,
    /// The cases of the Rust programs, in the order of the crate's programs and their cases.
    pub program_cases: Vec<ProgramCase>,
}

/// The bench of `krate`: a test program for each case of each function, function block and method
/// that Structured Text can call, `PUBLIC` or `INTERNAL`, inside the POU's namespace so that an
/// `INTERNAL` method can be called (specification 04). A POU without a case has the case of its
/// declared initial values. A program's cases give the inputs the bench writes into its
/// configured instances, since no Structured Text calls a program; a `PRIVATE` or `PROTECTED`
/// method has no measurement of its own (RU-48).
#[must_use]
pub fn programs(krate: &RustCrate) -> Bench {
    let mut bench = Bench::default();
    for rust in &krate.pous {
        let pou = &rust.pou;
        let (namespaces, local) = split(&pou.name);
        match pou.kind {
            Kind::Function | Kind::FunctionBlock => {
                for case in cases(&pou.bench) {
                    let call = match pou.kind {
                        Kind::Function => Call::Function(local),
                        _ => Call::Block(local),
                    };
                    add(
                        &mut bench,
                        &namespaces,
                        &pou.name,
                        call,
                        &pou.variables,
                        pou.result.as_ref(),
                        case,
                    );
                }
            }
            Kind::Program => {
                for case in cases(&pou.bench) {
                    bench.program_cases.push(ProgramCase {
                        pou: pou.name.clone(),
                        inputs: case
                            .values
                            .iter()
                            .map(|(name, value)| (name.clone(), value.structured_text()))
                            .collect(),
                        case: described(&case),
                    });
                }
            }
        }
        for method in &pou.methods {
            if !matches!(method.access, Access::Public | Access::Internal) {
                continue;
            }
            for case in cases(&method.bench) {
                add(
                    &mut bench,
                    &namespaces,
                    &format!("{}.{}", pou.name, method.name),
                    Call::Method {
                        block: local,
                        method: &method.name,
                    },
                    &method.variables,
                    method.result.as_ref(),
                    case,
                );
            }
        }
    }
    bench
}

/// What a test program calls.
enum Call<'a> {
    Function(&'a str),
    Block(&'a str),
    Method { block: &'a str, method: &'a str },
}

/// A POU's cases, or the one of its declared initial values.
fn cases(cases: &[Case]) -> Vec<Case> {
    if cases.is_empty() {
        vec![Case { values: Vec::new() }]
    } else {
        cases.to_vec()
    }
}

/// `Motion.AXIS` as its namespaces and its name in them.
fn split(name: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = name.split('.').collect();
    let local = parts.pop().unwrap_or(name);
    (parts, local)
}

/// Adds the test program of one case: a variable for each input the case names and for each
/// in-out, at the case's value, else at the in-out's declared initial value, which the call
/// passes; an output is not read, and an input the case does not name is left out of the call, so
/// that it keeps its declared initial value (IEC §6.6.1.4.2).
fn add(
    bench: &mut Bench,
    namespaces: &[&str],
    pou: &str,
    call: Call<'_>,
    variables: &[Variable],
    result: Option<&DataType>,
    case: Case,
) {
    let program = SmolStr::new(format!("TRUST_BENCH_{}", bench.programs.len() + 1));
    let mut declarations = String::new();
    let mut arguments = Vec::new();
    let given = |name: &str| {
        case.values
            .iter()
            .find(|(named, _)| named == name)
            .map(|(_, value)| value)
    };
    for (index, variable) in variables.iter().enumerate() {
        let value: Option<&Value> = match variable.section {
            Section::Input => match given(&variable.name) {
                Some(value) => Some(value),
                None => continue,
            },
            Section::InOut => given(&variable.name).or(variable.init.as_ref()),
            Section::Output | Section::Var => continue,
        };
        let local = format!("TrustBench{}", index + 1);
        let _ = write!(declarations, "    {local} : {}", variable.ty.iec());
        if let Some(value) = value {
            let _ = write!(declarations, " := {}", value.structured_text());
        }
        declarations.push_str(";\n");
        arguments.push(format!("{} := {local}", variable.name));
    }
    let arguments = arguments.join(", ");
    let statement = match call {
        Call::Function(name) => match result {
            Some(ty) => {
                let _ = writeln!(declarations, "    TrustBenchResult : {};", ty.iec());
                format!("TrustBenchResult := {name}({arguments});")
            }
            None => format!("{name}({arguments});"),
        },
        Call::Block(name) => {
            let _ = writeln!(declarations, "    TrustBenchBlock : {name};");
            format!("TrustBenchBlock({arguments});")
        }
        Call::Method { block, method } => {
            let _ = writeln!(declarations, "    TrustBenchBlock : {block};");
            match result {
                Some(ty) => {
                    let _ = writeln!(declarations, "    TrustBenchResult : {};", ty.iec());
                    format!("TrustBenchResult := TrustBenchBlock.{method}({arguments});")
                }
                None => format!("TrustBenchBlock.{method}({arguments});"),
            }
        }
    };
    let mut text = String::new();
    for namespace in namespaces {
        let _ = writeln!(text, "NAMESPACE {namespace}");
    }
    let _ = writeln!(text, "TEST_PROGRAM {program}");
    if !declarations.is_empty() {
        let _ = write!(text, "VAR\n{declarations}END_VAR\n");
    }
    let _ = write!(text, "{statement}\nEND_TEST_PROGRAM\n");
    for _ in namespaces {
        text.push_str("END_NAMESPACE\n");
    }
    text.push('\n');
    bench.source.push_str(&text);
    let qualified = if namespaces.is_empty() {
        program
    } else {
        SmolStr::new(format!("{}.{program}", namespaces.join(".")))
    };
    bench.programs.push(BenchProgram {
        program: qualified,
        pou: pou.to_string(),
        case: described(&case),
    });
}

/// `case` as it gives its values, `x = 1.0E6, alpha = 0.001`; empty for the declared initial
/// values.
fn described(case: &Case) -> String {
    case.values
        .iter()
        .map(|(name, value)| format!("{name} = {}", value.structured_text()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes `inputs`, each a Structured Text literal of its variable's type, into the program
/// instance `instance` of `program`, as the Live Values grammar admits a literal for the variable's
/// declared type (specification 11, "Live Values"): a Rust program's case before each scan
/// (RU-48).
///
/// # Errors
///
/// The text of an input the instance has no variable of, of a literal its type refuses, and of a
/// write the program refuses.
pub fn write_inputs(
    program: &mut NativeProgram<'_, HostCode>,
    instance: &str,
    inputs: &[(String, String)],
) -> Result<(), String> {
    for (input, text) in inputs {
        let path = format!("{instance}.{input}");
        let (index, value) = {
            let image = program.image();
            let (index, symbol) = image
                .find_symbol(&path.to_ascii_uppercase())
                .ok_or_else(|| format!("{path} is no variable of the program"))?;
            let value = trust_runtime_services::live_values::value_grammar::typed_value(
                &image,
                symbol.type_index,
                text,
            )
            .map_err(|error| format!("{path} := {text}: {error}"))?;
            (index, value)
        };
        program
            .write_symbol(index, &value)
            .map_err(|error| format!("{path} := {text}: {error}"))?;
    }
    Ok(())
}
