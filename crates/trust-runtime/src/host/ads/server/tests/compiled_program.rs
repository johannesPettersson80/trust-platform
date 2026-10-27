//! The ADS server of a compiled program (specification 44 §13.3): its symbols, reads and writes
//! reach the program's globals by their declared names through its symbol table.

use super::*;
use crate::runtime::CompiledVariable;

use super::super::publisher::read_symbol_bytes;
use super::super::symbols::build_compiled_symbol_snapshot;

/// A compiled program's variable as its symbol table lists it: `Program.var` for a program
/// instance's own variable, a bare name for a global.
fn compiled_variable(symbol: u32, name: &str, type_name: &str, instance: bool) -> CompiledVariable {
    let (program, member) = match name.split_once('.') {
        Some((program, member)) => (Some(SmolStr::new(program)), member),
        None => (None, name),
    };
    CompiledVariable {
        symbol,
        name: SmolStr::new(name),
        program,
        member: SmolStr::new(member),
        type_name: SmolStr::new(type_name),
        retained: false,
        constant: false,
        instance,
    }
}

/// Specification 44 §13.3 with specification 11's ADS server runtime publication: a compiled
/// program publishes its globals as an interpreted program does, `global.Speed` as an `INT` of 2
/// bytes, writable by the glob `Speed`. Catches a compiled program that publishes its program
/// variables, which an interpreted one does not, or an instance.
#[test]
fn compiled_symbols_publish_the_globals_as_an_interpreted_program_does() {
    let mut config = config(true);
    config.writable = vec![SmolStr::new("Speed")];
    // In the symbol table's order: the program instance's own variables, then the globals.
    let variables = [
        compiled_variable(1, "Vat.RegState", "INT", false),
        compiled_variable(2, "Vat.Delay", "TON", true),
        compiled_variable(3, "Speed", "INT", false),
    ];

    // The stand-in gives the instance a value too, so that only its instance flag leaves it out.
    let symbols = build_compiled_symbol_snapshot(&config, &variables, |_| Some(Value::Int(0)))
        .expect("symbol snapshot");

    let published = symbols
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.name.as_str(),
                symbol.data_type.iec_type,
                symbol.byte_size,
                symbol.flags.contains(&SymbolFlag::Write),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        published,
        vec![("global.Speed", IecDataType::Int, 2, true)],
        "a global publishes by its name, a program variable and an instance never"
    );
}

/// Specification 44 §13.3: an ADS read of a compiled program's global asks the program for the
/// global's declared name, and reads `INT` -300 as its two little-endian bytes. Catches a read by
/// the ADS symbol's name, fabricated bytes for a missing variable, and a foreign namespace stripped
/// to reach the program.
#[test]
fn compiled_read_takes_the_value_by_the_variable_name() {
    let config = config(false);
    let variables = [compiled_variable(1, "Speed", "INT", false)];
    let symbol = build_compiled_symbol_snapshot(&config, &variables, |_| Some(Value::Int(0)))
        .expect("symbol snapshot")
        .symbols
        .into_iter()
        .next()
        .expect("Speed symbol");
    let mut asked = Vec::new();

    let bytes = read_symbol_bytes(&config, &symbol, |name| {
        asked.push(name.to_string());
        Some(Value::Int(-300))
    })
    .expect("read");

    assert_eq!(
        asked,
        ["Speed"],
        "the value must be asked by the declared name, not by the ADS symbol name"
    );
    assert_eq!(
        bytes,
        [0xD4_u8, 0xFE],
        "INT -300 must read as its two little-endian bytes"
    );

    let missing = read_symbol_bytes(&config, &symbol, |_| None::<Value>)
        .expect_err("a variable the program does not hold");
    assert_eq!(
        missing.code(),
        AdsErrorCode::NotFound,
        "a missing variable must be NotFound, never fabricated bytes"
    );

    let mut foreign = symbol;
    foreign.name = "line_2.global.Speed".to_string();
    let foreign_error = read_symbol_bytes(&config, &foreign, |_| Some(Value::Int(1)))
        .expect_err("a foreign namespace");
    assert_eq!(
        foreign_error.code(),
        AdsErrorCode::NotFound,
        "a foreign namespace must not be stripped to reach the program's variable"
    );
}

/// Specification 44 §13.3: an ADS write to a compiled program's global queues the value by the
/// global's declared name, which the program resolves through its symbol table at the next cycle
/// start. Catches a write queued by the ADS symbol's name.
#[test]
fn compiled_write_queues_the_global_by_its_name() {
    let mut config = config(true);
    config.writable = vec![SmolStr::new("Speed")];
    let variables = [compiled_variable(1, "Speed", "INT", false)];
    let symbol = build_compiled_symbol_snapshot(&config, &variables, |_| Some(Value::Int(0)))
        .expect("symbol snapshot")
        .symbols
        .into_iter()
        .next()
        .expect("Speed symbol");
    let policy = AdsServerClientPolicy::new(&config);
    let debug = DebugControl::new();
    let write_port =
        AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource_control())
            .expect("write port");

    // INT 42 as its two little-endian bytes.
    write_port
        .write(&symbol, &[42, 0], &allowed_client())
        .expect("write");

    let pending = debug.drain_var_writes();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].target,
        PendingVarTarget::Global(SmolStr::new("Speed")),
        "the write must be queued by the declared name, which the program resolves through its \
         symbol table, not by the ADS symbol name"
    );
    assert_eq!(pending[0].value, Value::Int(42));
}
