//! Built-in import contracts (spec 34 §5.2.1, LOAD-06, PORT-02): the contract table agrees with
//! the standard-library registration entry by entry, every built-in has a contract, its failures
//! stay within that contract, and preparation refuses a missing or mismatched import by name
//! while still admitting the artifacts the compiler emits.

use std::panic::{catch_unwind, AssertUnwindSafe};

use trust_runtime_core::bytecode::{
    BytecodeModule, ParamEntry, SectionData, SectionId, StorageRole, TypeData,
};
use trust_runtime_core::error::{ImportRefusal, RuntimeError, StableErrorCode};
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::stdlib::contracts::{
    self, FbParameterType, FunctionSignature, ImportAllocation, ImportCapability, ImportContract,
    ImportWork,
};
use trust_runtime_core::stdlib::fbs::{self, BuiltinFbKind};
use trust_runtime_core::stdlib::{conversions, time, StandardLibrary, StdParams};
use trust_runtime_core::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const PROGRAM: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const NUMERIC: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
const CLOCK: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
/// The frozen registration list: every registered function and its parameter shape.
const REGISTRATION: &str = include_str!("../src/stdlib/registration-baseline.txt");

/// The registered families by IEC 61131-3 Ed.3 table, with the charges of their contract.
const FAMILIES: [(&str, &str, ImportWork, ImportAllocation); 8] = [
    (
        "numeric, IEC Tables 28 and 29",
        "ABS ACOS ADD ASIN ATAN ATAN2 COS DIV EXP EXPT LN LOG MOD MOVE MUL SIN SQRT SUB TAN",
        ImportWork::TextArguments,
        ImportAllocation::TextResult,
    ),
    (
        "bit, IEC Tables 30 and 31",
        "AND NOT OR ROL ROR SHL SHR XOR",
        ImportWork::TextArguments,
        ImportAllocation::NoText,
    ),
    (
        "selection, IEC Table 32",
        "LIMIT MAX MIN MUX SEL",
        ImportWork::TextArguments,
        ImportAllocation::TextResult,
    ),
    (
        "comparison, IEC Table 33",
        "EQ GE GT LE LT NE",
        ImportWork::TextArguments,
        ImportAllocation::NoText,
    ),
    (
        "string, IEC Table 34",
        "CONCAT DELETE FIND INSERT LEFT LEN MID REPLACE RIGHT __TRUST_LIMIT_STRING",
        ImportWork::TextArguments,
        ImportAllocation::TextResult,
    ),
    (
        "time, IEC Tables 35 and 36",
        "ADD_DT_TIME ADD_LDT_LTIME ADD_LTIME ADD_LTOD_LTIME ADD_TIME ADD_TOD_TIME CONCAT_DATE \
         CONCAT_DATE_LTOD CONCAT_DATE_TOD CONCAT_DT CONCAT_LDT CONCAT_LTOD CONCAT_TOD DAY_OF_WEEK \
         DIV_LTIME DIV_TIME MUL_LTIME MUL_TIME SUB_DATE_DATE SUB_DT_DT SUB_DT_TIME SUB_LDATE_LDATE \
         SUB_LDT_LDT SUB_LDT_LTIME SUB_LTIME SUB_LTOD_LTIME SUB_LTOD_LTOD SUB_TIME SUB_TOD_TIME \
         SUB_TOD_TOD",
        ImportWork::TextArguments,
        ImportAllocation::NoText,
    ),
    (
        "validate, IEC Table 39",
        "IS_VALID IS_VALID_BCD",
        ImportWork::TextArguments,
        ImportAllocation::NoText,
    ),
    (
        "assertions, specification 07 (not IEC)",
        "ASSERT_EQUAL ASSERT_FALSE ASSERT_GREATER ASSERT_GREATER_OR_EQUAL ASSERT_LESS \
         ASSERT_LESS_OR_EQUAL ASSERT_NEAR ASSERT_NOT_EQUAL ASSERT_TRUE",
        ImportWork::TextArguments,
        ImportAllocation::NoText,
    ),
];

/// IEC Tables 43 to 46: every standard function-block name, its implementation, and the
/// STBC primitive ids of its counter value and its duration.
const FUNCTION_BLOCKS: [(&str, BuiltinFbKind, u16, u16); 33] = [
    ("RS", BuiltinFbKind::Rs, 0x0100, 16),
    ("SR", BuiltinFbKind::Sr, 0x0100, 16),
    ("R_TRIG", BuiltinFbKind::RTrig, 0x0100, 16),
    ("F_TRIG", BuiltinFbKind::FTrig, 0x0100, 16),
    ("DIFU", BuiltinFbKind::RTrig, 0x0100, 16),
    ("DIFD", BuiltinFbKind::FTrig, 0x0100, 16),
    ("CTU", BuiltinFbKind::Ctu, 0x0100, 16),
    ("CTU_INT", BuiltinFbKind::Ctu, 7, 16),
    ("CTU_DINT", BuiltinFbKind::Ctu, 8, 16),
    ("CTU_LINT", BuiltinFbKind::Ctu, 9, 16),
    ("CTU_UDINT", BuiltinFbKind::Ctu, 12, 16),
    ("CTU_ULINT", BuiltinFbKind::Ctu, 13, 16),
    ("CTD", BuiltinFbKind::Ctd, 0x0100, 16),
    ("CTD_INT", BuiltinFbKind::Ctd, 7, 16),
    ("CTD_DINT", BuiltinFbKind::Ctd, 8, 16),
    ("CTD_LINT", BuiltinFbKind::Ctd, 9, 16),
    ("CTD_UDINT", BuiltinFbKind::Ctd, 12, 16),
    ("CTD_ULINT", BuiltinFbKind::Ctd, 13, 16),
    ("CTUD", BuiltinFbKind::Ctud, 0x0100, 16),
    ("CTUD_INT", BuiltinFbKind::Ctud, 7, 16),
    ("CTUD_DINT", BuiltinFbKind::Ctud, 8, 16),
    ("CTUD_LINT", BuiltinFbKind::Ctud, 9, 16),
    ("CTUD_UDINT", BuiltinFbKind::Ctud, 12, 16),
    ("CTUD_ULINT", BuiltinFbKind::Ctud, 13, 16),
    ("TP", BuiltinFbKind::Tp, 0x0100, 16),
    ("TON", BuiltinFbKind::Ton, 0x0100, 16),
    ("TOF", BuiltinFbKind::Tof, 0x0100, 16),
    ("TP_TIME", BuiltinFbKind::Tp, 0x0100, 16),
    ("TON_TIME", BuiltinFbKind::Ton, 0x0100, 16),
    ("TOF_TIME", BuiltinFbKind::Tof, 0x0100, 16),
    ("TP_LTIME", BuiltinFbKind::Tp, 0x0100, 17),
    ("TON_LTIME", BuiltinFbKind::Ton, 0x0100, 17),
    ("TOF_LTIME", BuiltinFbKind::Tof, 0x0100, 17),
];

/// IEC formal parameters of each standard function block: name, output, type.
fn iec_parameters(kind: BuiltinFbKind) -> &'static [(&'static str, bool, FbParameterType)] {
    use FbParameterType::{Bool, Count, Duration};
    match kind {
        BuiltinFbKind::Rs => &[("S", false, Bool), ("R1", false, Bool), ("Q1", true, Bool)],
        BuiltinFbKind::Sr => &[("S1", false, Bool), ("R", false, Bool), ("Q1", true, Bool)],
        BuiltinFbKind::RTrig | BuiltinFbKind::FTrig => &[("CLK", false, Bool), ("Q", true, Bool)],
        BuiltinFbKind::Ctu => &[
            ("CU", false, Bool),
            ("R", false, Bool),
            ("PV", false, Count),
            ("Q", true, Bool),
            ("CV", true, Count),
        ],
        BuiltinFbKind::Ctd => &[
            ("CD", false, Bool),
            ("LD", false, Bool),
            ("PV", false, Count),
            ("Q", true, Bool),
            ("CV", true, Count),
        ],
        BuiltinFbKind::Ctud => &[
            ("CU", false, Bool),
            ("CD", false, Bool),
            ("R", false, Bool),
            ("LD", false, Bool),
            ("PV", false, Count),
            ("QU", true, Bool),
            ("QD", true, Bool),
            ("CV", true, Count),
        ],
        BuiltinFbKind::Tp | BuiltinFbKind::Ton | BuiltinFbKind::Tof => &[
            ("IN", false, Bool),
            ("PT", false, Duration),
            ("Q", true, Bool),
            ("ET", true, Duration),
        ],
    }
}

/// The version-1 hidden native state of each standard function block (specification 12
/// §11.5.5): slot name and STBC primitive id.
fn native_state(kind: BuiltinFbKind) -> &'static [(&'static str, u16)] {
    match kind {
        BuiltinFbKind::Rs | BuiltinFbKind::Sr => &[],
        BuiltinFbKind::RTrig | BuiltinFbKind::FTrig => &[("__ST_TRIG_M", 1)],
        BuiltinFbKind::Ctu => &[("__ST_PREV_CU", 1)],
        BuiltinFbKind::Ctd => &[("__ST_PREV_CD", 1)],
        BuiltinFbKind::Ctud => &[("__ST_PREV_CU", 1), ("__ST_PREV_CD", 1)],
        BuiltinFbKind::Ton => &[("__ST_LAST_TIME", 17)],
        BuiltinFbKind::Tof => &[
            ("__ST_LAST_TIME", 17),
            ("__ST_PREV_IN", 1),
            ("__ST_TIMING", 1),
        ],
        BuiltinFbKind::Tp => &[
            ("__ST_LAST_TIME", 17),
            ("__ST_PREV_IN", 1),
            ("__ST_ACTIVE", 1),
        ],
    }
}

fn signature_shape(params: &StdParams) -> String {
    fn names(fixed: &[impl AsRef<str>]) -> String {
        fixed
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join(",")
    }
    match params {
        StdParams::Fixed(fixed) => format!("fixed({})", names(&fixed[..])),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => format!("variadic({};{prefix};{start},{min})", names(&fixed[..])),
    }
}

#[test]
fn contract_table_and_registration_agree_entry_by_entry() {
    let library = StandardLibrary::new();
    let table: Vec<_> = contracts::registered().collect();
    assert_eq!(
        table.len(),
        REGISTRATION.lines().count(),
        "one contract entry per registered function"
    );
    for line in REGISTRATION.lines() {
        let (name, shape) = line.split_once(' ').expect("baseline line: name and shape");
        let entries: Vec<_> = table.iter().filter(|(entry, _)| *entry == name).collect();
        assert_eq!(entries.len(), 1, "{name} has exactly one contract entry");
        let (_, import) = entries[0];
        let FunctionSignature::Registered(params) = import.signature else {
            panic!("{name} is a registered signature")
        };
        assert_eq!(
            signature_shape(params),
            shape,
            "{name} keeps its registered parameters"
        );
        let registered = library.get(name).expect("registered function");
        assert!(
            std::ptr::eq(params, registered.params),
            "{name} borrows the registration's own parameter list"
        );
        assert_eq!(import.contract.version, 1, "{name}");
        let resolved = contracts::function(name).expect("resolves by name");
        assert_eq!(resolved.signature, import.signature, "{name}");
        assert!(std::ptr::eq(resolved.contract, import.contract), "{name}");
    }
    for (name, _) in &table {
        assert_eq!(
            REGISTRATION
                .lines()
                .filter(|line| line.split(' ').next() == Some(*name))
                .count(),
            1,
            "{name} is registered once"
        );
    }

    // Each IEC family shares one contract with its family's charges.
    let mut members = 0;
    for (family, names, work, allocation) in FAMILIES {
        let names: Vec<&str> = names.split_whitespace().collect();
        let contract = contracts::function(names[0])
            .expect("family member")
            .contract;
        for name in &names {
            let import = contracts::function(name).unwrap_or_else(|| panic!("{family}: {name}"));
            assert!(
                std::ptr::eq(import.contract, contract),
                "{family}: {name} has its family's contract"
            );
        }
        assert_eq!(
            (
                contract.version,
                contract.work,
                contract.allocation,
                contract.capability
            ),
            (1, work, allocation, None),
            "{family}"
        );
        members += names.len();
    }
    assert_eq!(
        members,
        table.len(),
        "the IEC families cover every registered function"
    );
}

#[test]
fn every_split_clock_conversion_and_function_block_has_a_contract() {
    for name in [
        "SPLIT_DATE",
        "SPLIT_TOD",
        "SPLIT_LTOD",
        "SPLIT_DT",
        "SPLIT_LDT",
    ] {
        assert!(time::is_split_name(name), "{name}");
        let import = contracts::function(name).expect("split contract");
        let FunctionSignature::Split(params) = import.signature else {
            panic!("{name} is a split signature")
        };
        assert_eq!(Some(params), time::split_parameter_names(name), "{name}");
        assert_eq!(params[0], "IN", "{name} takes its input first");
        assert_eq!(
            (
                import.contract.version,
                import.contract.work,
                import.contract.allocation,
                import.contract.capability
            ),
            (1, ImportWork::Binding, ImportAllocation::NoText, None),
            "{name}"
        );
    }
    for (name, capability) in [
        ("TIME", None),
        ("CURRENT_DT", Some(ImportCapability::WallClock)),
    ] {
        assert!(time::is_runtime_clock_name(name), "{name}");
        let import = contracts::function(name).expect("clock contract");
        assert_eq!(import.signature, FunctionSignature::Clock, "{name}");
        assert_eq!(import.contract.capability, capability, "{name}");
        assert_eq!(import.contract.work, ImportWork::Binding, "{name}");
    }

    // Every name a Table 22 conversion form recognizes resolves to the one conversion contract.
    let conversion = contracts::function("INT_TO_DINT")
        .expect("conversion contract")
        .contract;
    let mut recognized = 0;
    for name in conversion_names() {
        match contracts::function(&name) {
            Some(import) if conversions::is_conversion_name(&name) => {
                assert_eq!(import.signature, FunctionSignature::Conversion, "{name}");
                assert!(std::ptr::eq(import.contract, conversion), "{name}");
                recognized += 1;
            }
            None => assert!(!conversions::is_conversion_name(&name), "{name}"),
            Some(import) => panic!("{name} is not a conversion but resolves to {import:?}"),
        }
    }
    assert!(recognized > 3_000, "{recognized} conversion names");
    for name in [
        "INT_TO_FOO",
        "TO_",
        "SQRTX",
        "CURRENT_DX",
        "SPLIT_DAY",
        "TON",
        "",
    ] {
        assert_eq!(contracts::function(name), None, "{name}");
    }

    // IEC Tables 43 to 46: parameters, their types per variant, and the hidden native state.
    for (name, kind, count, duration) in FUNCTION_BLOCKS {
        assert_eq!(fbs::builtin_kind(name), Some(kind), "{name}");
        let import = contracts::function_block(name).expect("function-block contract");
        assert_eq!(import.kind, kind, "{name}");
        let parameters: Vec<_> = import
            .parameters
            .iter()
            .map(|parameter| (parameter.name, parameter.output, parameter.ty))
            .collect();
        assert_eq!(parameters, iec_parameters(kind), "{name}");
        for parameter in import.parameters {
            let expected = match parameter.ty {
                FbParameterType::Bool => 1,
                FbParameterType::Count => count,
                FbParameterType::Duration => duration,
            };
            assert_eq!(
                import.primitive(parameter.ty),
                expected,
                "{name}.{}",
                parameter.name
            );
        }
        assert_eq!(import.state, native_state(kind), "{name}");
        assert_eq!(
            (
                import.contract.version,
                import.contract.work,
                import.contract.allocation,
                import.contract.failures,
                import.contract.capability
            ),
            (
                1,
                ImportWork::Binding,
                ImportAllocation::NoText,
                &[StableErrorCode::RuntimeTypeMismatch][..],
                None
            ),
            "{name}"
        );
    }
    for name in ["TON2", "COUNTER", "SQRT", "ton"] {
        assert_eq!(contracts::function_block(name), None, "{name}");
    }
}

/// Elementary and generic type names a conversion name may carry (IEC Tables 22 to 27).
const CONVERSION_TYPES: &str = "BOOL SINT INT DINT LINT USINT UINT UDINT ULINT REAL LREAL BYTE \
    WORD DWORD LWORD TIME LTIME DATE LDATE TOD TIME_OF_DAY LTOD LTIME_OF_DAY DT DATE_AND_TIME LDT \
    LDATE_AND_TIME STRING WSTRING CHAR WCHAR ANY_INT ANY_NUM";

/// Names in every conversion form of IEC Table 22 over the elementary types.
fn conversion_names() -> Vec<String> {
    let mut names = vec!["TRUNC".to_string()];
    for target in CONVERSION_TYPES.split_whitespace() {
        names.push(format!("TO_{target}"));
        names.push(format!("TRUNC_{target}"));
        names.push(format!("BCD_TO_{target}"));
        names.push(format!("TO_BCD_{target}"));
        for source in CONVERSION_TYPES.split_whitespace() {
            names.push(format!("{source}_TO_{target}"));
            names.push(format!("{source}_TRUNC_{target}"));
            names.push(format!("{source}_BCD_TO_{target}"));
            names.push(format!("{source}_TO_BCD_{target}"));
        }
    }
    names
}

/// Scalar values at and around the limits of every elementary type, wrong types included.
fn corpus() -> Vec<Value> {
    let nanos = Duration::from_nanos;
    let mut values = vec![Value::Null, Value::Bool(false), Value::Bool(true)];
    values.extend([i8::MIN, -1, 0, 3, i8::MAX].map(Value::SInt));
    values.extend([i16::MIN, -1, 0, 2, i16::MAX].map(Value::Int));
    values.extend([i32::MIN, 0, 1, i32::MAX].map(Value::DInt));
    values.extend([i64::MIN, -3, 0, i64::MAX].map(Value::LInt));
    values.extend([0, u8::MAX].map(Value::USInt));
    values.extend([0, 7, u16::MAX].map(Value::UInt));
    values.extend([0, u32::MAX].map(Value::UDInt));
    values.extend([0, 1, u64::MAX].map(Value::ULInt));
    values.extend([0.0, -1.5, 2.0, f32::MAX, f32::NAN, f32::INFINITY].map(Value::Real));
    values.extend([0.0, -0.5, 0.5, 4.0, f64::MAX, f64::NAN, f64::NEG_INFINITY].map(Value::LReal));
    values.extend([0, 0x12, 0xFF].map(Value::Byte));
    values.extend([
        Value::Word(0x1234),
        Value::DWord(u32::MAX),
        Value::LWord(u64::MAX),
    ]);
    values.extend([0, 1_500_000_000, i64::MAX, i64::MIN].map(|n| Value::Time(nanos(n))));
    values.extend([7, i64::MAX].map(|n| Value::LTime(nanos(n))));
    values.extend([0, i64::MAX, i64::MIN].map(|t| Value::Date(DateValue::new(t))));
    values.extend([0, i64::MAX].map(|t| Value::LDate(LDateValue::new(t))));
    values.extend([0, i64::MAX].map(|t| Value::Tod(TimeOfDayValue::new(t))));
    values.extend([1, i64::MIN].map(|t| Value::LTod(LTimeOfDayValue::new(t))));
    values.extend([0, i64::MIN, i64::MAX].map(|t| Value::Dt(DateTimeValue::new(t))));
    values.extend([-1, i64::MAX].map(|t| Value::Ldt(LDateTimeValue::new(t))));
    values.extend(["", "abc", "16#FF", "-1.5e3", "x"].map(|text| Value::String(text.into())));
    values.extend(["", "äbc"].map(|text| Value::WString(text.into())));
    values.extend([
        Value::Char(b'a'),
        Value::WChar(0x00E4),
        Value::WChar(0xD800),
    ]);
    values
}

/// Deterministic xorshift sampler for argument vectors longer than two.
struct Sampler(u32);

impl Sampler {
    fn index(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as usize % bound
    }
}

/// Every argument vector of `arity` values: all of them up to two values, a fixed sample beyond.
fn argument_vectors(corpus: &[Value], arity: usize, sampler: &mut Sampler) -> Vec<Vec<Value>> {
    match arity {
        0 => vec![Vec::new()],
        1 => corpus.iter().map(|value| vec![value.clone()]).collect(),
        2 => corpus
            .iter()
            .flat_map(|left| corpus.iter().map(|right| vec![left.clone(), right.clone()]))
            .collect(),
        _ => (0..3_000)
            .map(|_| {
                (0..arity)
                    .map(|_| corpus[sampler.index(corpus.len())].clone())
                    .collect()
            })
            .collect(),
    }
}

/// Run one call, recording a panic as an outcome instead of ending the test.
fn probe(
    call: impl FnOnce() -> Result<(), RuntimeError>,
) -> std::thread::Result<Result<(), RuntimeError>> {
    catch_unwind(AssertUnwindSafe(call))
}

/// Failures outside a contract, with the first few kept for the report.
#[derive(Default)]
struct Violations {
    count: usize,
    sample: Vec<String>,
}

impl Violations {
    fn check(
        &mut self,
        import: &str,
        contract: &ImportContract,
        input: &dyn core::fmt::Debug,
        outcome: std::thread::Result<Result<(), RuntimeError>>,
    ) {
        let violation = match outcome {
            Ok(Ok(())) => return,
            Ok(Err(error)) if contract.failures.contains(&error.stable_code()) => return,
            Ok(Err(error)) => format!("{import}{input:?} -> {error:?}"),
            Err(_) => format!("{import}{input:?} panicked"),
        };
        self.count += 1;
        if self.sample.len() < 40 {
            self.sample.push(violation);
        }
    }

    fn assert_none(self, what: &str) {
        assert_eq!(
            self.count, 0,
            "{what}: failures outside the contract {:#?}",
            self.sample
        );
    }
}

#[test]
fn registered_function_failures_stay_within_their_contracts() {
    let corpus = corpus();
    let library = StandardLibrary::new();
    let mut sampler = Sampler(0x0c4e_2026);
    let mut violations = Violations::default();
    let mut calls = 0usize;
    for (name, import) in contracts::registered() {
        let FunctionSignature::Registered(params) = import.signature else {
            panic!("{name} is registered")
        };
        let arities = match params {
            StdParams::Fixed(fixed) => vec![fixed.len()],
            StdParams::Variadic { fixed, min, .. } => {
                vec![fixed.len() + min, fixed.len() + min + 1]
            }
        };
        let function = library.get(name).expect("registered").func;
        for arity in arities {
            for args in argument_vectors(&corpus, arity, &mut sampler) {
                let outcome = probe(|| function(&args).map(drop));
                violations.check(name, import.contract, &args, outcome);
                calls += 1;
            }
        }
    }
    assert!(calls > 300_000, "{calls} calls");
    violations.assert_none("registered functions");
}

#[test]
fn conversion_split_clock_and_function_block_failures_stay_within_their_contracts() {
    let corpus = corpus();
    let mut violations = Violations::default();

    let conversion = contracts::function("INT_TO_DINT").unwrap().contract;
    for name in conversion_names() {
        let Some(spec) = conversions::conversion_spec(&name) else {
            continue;
        };
        for value in &corpus {
            let args = [value.clone()];
            let outcome = probe(|| conversions::call_conversion_spec(spec, &args).map(drop));
            violations.check(&name, conversion, &args, outcome);
        }
    }

    let split = contracts::function("SPLIT_DATE").unwrap().contract;
    let profiles = [
        DateTimeProfile::default(),
        DateTimeProfile {
            resolution: Duration::ZERO,
            ..DateTimeProfile::default()
        },
        DateTimeProfile {
            epoch: DateValue::new(i64::MAX),
            ..DateTimeProfile::default()
        },
    ];
    for value in &corpus {
        for profile in profiles {
            let outcomes = [
                (
                    "SPLIT_DATE",
                    probe(|| time::split_date(value, profile).map(drop)),
                ),
                (
                    "SPLIT_TOD",
                    probe(|| time::split_tod(value, profile).map(drop)),
                ),
                ("SPLIT_LTOD", probe(|| time::split_ltod(value).map(drop))),
                (
                    "SPLIT_DT",
                    probe(|| time::split_dt(value, profile).map(drop)),
                ),
                ("SPLIT_LDT", probe(|| time::split_ldt(value).map(drop))),
            ];
            for (name, outcome) in outcomes {
                violations.check(name, split, value, outcome);
            }
        }
    }

    let elapsed = contracts::function("TIME").unwrap().contract;
    let wall_clock = contracts::function("CURRENT_DT").unwrap().contract;
    for nanos in [0, 1, i64::MAX, i64::MIN] {
        let outcome = probe(|| {
            time::runtime_clock_value("TIME", Duration::from_nanos(nanos), || {
                unreachable!("TIME reads no wall clock")
            })
            .map(drop)
        });
        violations.check("TIME", elapsed, &nanos, outcome);
    }
    for since_epoch in [
        core::time::Duration::ZERO,
        core::time::Duration::from_secs(1_760_000_000),
        core::time::Duration::MAX,
    ] {
        let outcome = probe(|| {
            time::runtime_clock_value("CURRENT_DT", Duration::ZERO, || {
                time::current_dt_elapsed(since_epoch)
            })
            .map(drop)
        });
        violations.check("CURRENT_DT", wall_clock, &since_epoch, outcome);
    }

    // Function blocks run against instance fields holding any corpus value, or none.
    let mut sampler = Sampler(0x5eed_0c42);
    for (name, kind, _, _) in FUNCTION_BLOCKS {
        let import = contracts::function_block(name).unwrap();
        let fields: Vec<&str> = import
            .parameters
            .iter()
            .map(|parameter| parameter.name)
            .chain(import.state.iter().map(|(slot, _)| *slot))
            .collect();
        let mut storage = VariableStorage::default();
        for _ in 0..2_000 {
            let instance = storage
                .try_create_instance(name)
                .expect("instance identity");
            let mut assigned = Vec::new();
            for field in &fields {
                let pick = sampler.index(corpus.len() + 1);
                if let Some(value) = corpus.get(pick) {
                    assert!(storage.set_instance_var(instance, *field, value.clone()));
                    assigned.push((*field, value.clone()));
                }
            }
            let now = Duration::from_nanos([0, 1, 5_000_000, i64::MAX, i64::MIN][sampler.index(5)]);
            let outcome =
                probe(|| fbs::execute_builtin_in_storage(&mut storage, now, instance, kind));
            violations.check(name, import.contract, &(now, &assigned), outcome);
        }
    }

    violations.assert_none("conversions, splits, clocks and function blocks");
}

#[test]
fn a_string_result_above_the_implementer_maximum_is_a_contract_failure() {
    // IEC 61131-3 Ed.3 §6.6.2.5.11 and docs/IEC_DECISIONS.md 2026-10-17: a text result longer
    // than 65,535 bytes or code units is an error, which the String and Conversion contracts name.
    let half = Value::String("x".repeat(40_000).into());
    let concat = StandardLibrary::new()
        .get("CONCAT")
        .expect("registered")
        .func;
    let error = concat(&[half.clone(), half]).expect_err("80,000 bytes");
    assert_eq!(error, RuntimeError::StringCapacityExceeded);
    let strings = contracts::function("CONCAT").unwrap().contract;
    assert!(strings.failures.contains(&error.stable_code()));

    // 30,000 code units of "€" are 90,000 bytes of UTF-8.
    let euros = [Value::WString("€".repeat(30_000))];
    let spec = conversions::conversion_spec("WSTRING_TO_STRING").expect("a conversion");
    let error = conversions::call_conversion_spec(spec, &euros).expect_err("90,000 bytes");
    assert_eq!(error, RuntimeError::StringCapacityExceeded);
    let conversion = contracts::function("WSTRING_TO_STRING").unwrap().contract;
    assert!(conversion.failures.contains(&error.stable_code()));
}

fn decoded(bytes: &[u8]) -> BytecodeModule {
    BytecodeModule::decode(bytes).expect("saved artifact decodes")
}

fn string_index(module: &BytecodeModule, text: &str) -> u32 {
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("STRING_TABLE")
    };
    strings
        .entries
        .iter()
        .position(|entry| entry.as_str() == text)
        .unwrap_or_else(|| panic!("saved string {text}")) as u32
}

/// Replace a native-call descriptor of the saved artifact; every call site using it changes.
fn rename_symbol(module: &mut BytecodeModule, from: &str, to: &str) {
    let index = string_index(module, from) as usize;
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("STRING_TABLE")
    };
    strings.entries[index] = to.into();
}

/// A declared parameter of a POU template in the saved artifact.
fn parameter<'a>(module: &'a mut BytecodeModule, pou: &str, name: &str) -> &'a mut ParamEntry {
    let pou_name = string_index(module, pou);
    let name = string_index(module, name);
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("POU_INDEX")
    };
    index
        .entries
        .iter_mut()
        .find(|entry| entry.name_idx == pou_name)
        .expect("saved POU template")
        .params
        .iter_mut()
        .find(|param| param.name_idx == name)
        .expect("saved parameter")
}

fn primitive_type(module: &BytecodeModule, prim: u16) -> u32 {
    let Some(SectionData::TypeTable(types)) = module.section(SectionId::TypeTable) else {
        panic!("TYPE_TABLE")
    };
    types
        .entries
        .iter()
        .position(|entry| {
            matches!(entry.data, TypeData::Primitive { prim_id, max_length: 0 } if prim_id == prim)
        })
        .expect("saved primitive type") as u32
}

/// The mutation stays a valid artifact, and both preparation routes refuse it alike before
/// any state exists.
fn refusal(module: &BytecodeModule) -> RuntimeError {
    module
        .validated_source_free(Default::default())
        .expect("the mutation keeps the wire format valid");
    let decoded = PreparedModule::from_decoded(module, PreparationLimits::default())
        .expect_err("preparation refuses the import");
    let bytes = module.encode().expect("encode the mutated artifact");
    let from_bytes = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
        .expect_err("preparation refuses the import");
    assert_eq!(decoded, from_bytes, "both preparation routes refuse alike");
    decoded
}

fn assert_refused(error: RuntimeError, import: &str, reason: ImportRefusal) {
    assert_eq!(
        error,
        RuntimeError::ImportUnsupported {
            import: import.into(),
            reason,
        }
    );
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeImportUnsupported
    );
    assert_eq!(error.stable_code().as_str(), "runtime_import_unsupported");
    assert!(
        error.to_string().contains(&format!("'{import}'")),
        "{error}"
    );
}

#[test]
fn a_missing_import_is_refused_by_name_before_any_state_exists() {
    let mut numeric = decoded(NUMERIC);
    rename_symbol(&mut numeric, "SQRT|E", "SQRTX|E");
    assert_refused(refusal(&numeric), "SQRTX", ImportRefusal::NoContract);

    let mut program = decoded(PROGRAM);
    rename_symbol(&mut program, "DINT_TO_INT|E", "UNAPPROVED_NATIVE|E");
    assert_refused(
        refusal(&program),
        "UNAPPROVED_NATIVE",
        ImportRefusal::NoContract,
    );

    // The refusal names the import by its canonical uppercase name.
    let mut clock = decoded(CLOCK);
    rename_symbol(&mut clock, "CURRENT_DT", "current_dx");
    assert_refused(refusal(&clock), "CURRENT_DX", ImportRefusal::NoContract);
}

#[test]
fn a_mismatched_import_arity_is_refused_by_name() {
    for (symbol, import) in [
        ("ATAN2|E", "ATAN2"),
        ("ADD|E", "ADD"),
        ("TIME|E", "TIME"),
        ("SPLIT_TOD|E", "SPLIT_TOD"),
    ] {
        let mut numeric = decoded(NUMERIC);
        rename_symbol(&mut numeric, "SQRT|E", symbol);
        assert_refused(refusal(&numeric), import, ImportRefusal::ParameterCount);
    }
    let mut clock = decoded(CLOCK);
    rename_symbol(&mut clock, "CURRENT_DT", "SQRT");
    assert_refused(refusal(&clock), "SQRT", ImportRefusal::ParameterCount);

    let mut program = decoded(PROGRAM);
    let ton = string_index(&program, "TON");
    let Some(SectionData::PouIndex(index)) = program.section_mut(SectionId::PouIndex) else {
        panic!("POU_INDEX")
    };
    let template = index
        .entries
        .iter_mut()
        .find(|entry| entry.name_idx == ton)
        .expect("saved TON template");
    assert_eq!(template.params.len(), 4);
    template.params.pop();
    assert_refused(refusal(&program), "TON", ImportRefusal::ParameterCount);
}

#[test]
fn a_mismatched_import_type_or_name_is_refused_by_name() {
    let mut program = decoded(PROGRAM);
    let int = primitive_type(&program, 7);
    parameter(&mut program, "TON", "PT").type_id = int;
    assert_refused(refusal(&program), "TON", ImportRefusal::ParameterType);

    let mut program = decoded(PROGRAM);
    parameter(&mut program, "TON", "Q").direction = 0;
    assert_refused(refusal(&program), "TON", ImportRefusal::ParameterType);

    let mut program = decoded(PROGRAM);
    let q = string_index(&program, "Q");
    parameter(&mut program, "TON", "PT").name_idx = q;
    assert_refused(refusal(&program), "TON", ImportRefusal::ParameterName);

    let mut program = decoded(PROGRAM);
    let alternate = string_index(&program, "result");
    let Some(SectionData::StorageLayout(layout)) = program.section_mut(SectionId::StorageLayout)
    else {
        panic!("STORAGE_LAYOUT")
    };
    layout
        .entries
        .iter_mut()
        .find(|entry| entry.role == StorageRole::NativeState)
        .expect("TON state")
        .name_idx = alternate;
    assert_refused(refusal(&program), "TON", ImportRefusal::StateLayout);

    let mut numeric = decoded(NUMERIC);
    rename_symbol(&mut numeric, "SQRT|E", "SQRT|E:X");
    assert_refused(refusal(&numeric), "SQRT", ImportRefusal::ParameterName);

    let mut program = decoded(PROGRAM);
    rename_symbol(&mut program, "DINT_TO_INT|E", "DINT_TO_INT|E:OUT");
    assert_refused(
        refusal(&program),
        "DINT_TO_INT",
        ImportRefusal::ParameterName,
    );
}

#[test]
fn artifacts_the_compiler_emits_are_still_admitted() {
    for (artifact, bytes, imports) in [
        (
            "numeric-v2",
            NUMERIC,
            &[
                "SQRT", "LN", "LOG", "EXP", "SIN", "COS", "TAN", "ASIN", "ACOS", "ATAN",
            ][..],
        ),
        ("program-v2", PROGRAM, &["DINT_TO_INT"][..]),
        ("clock-v2", CLOCK, &["CURRENT_DT", "TIME"][..]),
        ("gpio", GPIO, &[][..]),
    ] {
        PreparedModule::from_bytes(bytes, PreparationLimits::default())
            .unwrap_or_else(|error| panic!("{artifact} is admitted: {error}"));
        for import in imports {
            assert!(
                contracts::function(import).is_some(),
                "{artifact} imports {import}"
            );
        }
    }

    // The compiler's TON template is exactly the contract's.
    let program = decoded(PROGRAM);
    let import = contracts::function_block("TON").unwrap();
    let ton = string_index(&program, "TON");
    let (Some(SectionData::PouIndex(index)), Some(SectionData::StringTable(strings))) = (
        program.section(SectionId::PouIndex),
        program.section(SectionId::StringTable),
    ) else {
        panic!("POU_INDEX and STRING_TABLE")
    };
    let template = index
        .entries
        .iter()
        .find(|entry| entry.name_idx == ton)
        .unwrap();
    let declared: Vec<_> = template
        .params
        .iter()
        .map(|param| {
            (
                strings.entries[param.name_idx as usize].as_str(),
                param.direction == 1,
            )
        })
        .collect();
    let contracted: Vec<_> = import
        .parameters
        .iter()
        .map(|parameter| (parameter.name, parameter.output))
        .collect();
    assert_eq!(declared, contracted);

    // CURRENT_DT's capability still decides the wall-clock check at instantiation.
    let clock = PreparedModule::from_bytes(CLOCK, PreparationLimits::default()).unwrap();
    let Err(error) = clock.instantiate(0) else {
        panic!("a logical-only composition has no wall clock")
    };
    let RuntimeError::ProfileUnsupported(reason) = &error else {
        panic!("{error}")
    };
    assert!(reason.contains("CURRENT_DT"), "{error}");
}
