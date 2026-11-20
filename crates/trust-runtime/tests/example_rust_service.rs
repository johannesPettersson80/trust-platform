//! The example `rust_service` (specification 48 RU-88): its program takes the price service's
//! setpoint only while the service's heartbeat is fresh (RU-63, specification 08 §5A), with the
//! values its README gives, and the service's committed bindings are the ones `trust-dev service
//! bindings` writes from the program (RU-62). The tests build a copy of the example with the
//! toolchain at `TRUST_TOOLCHAIN`, as `trust-runtime build` builds the project; the repository is
//! never written.

mod rust_pous_support;

use rust_pous_support::{built_from, example_copy, project_sources};
use trust_runtime::io::IoAddress;
use trust_runtime::native::rust_pous::service_writes;
use trust_runtime::native::service_bindings::service_bindings;
use trust_runtime::native::{load, HostCode, NativeProgram, NoHooks};
use trust_runtime::runtime_core::native::image::Image;
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The task's interval, in milliseconds.
const INTERVAL_MS: i64 = 100;

/// Counts the service's heartbeat to `count` in its global `price_heartbeat`, as the runtime does at
/// a cycle's start for each `service.heartbeat`.
fn beat(program: &mut NativeProgram<'_, HostCode>, count: u32) {
    let (symbol, _) = program
        .image()
        .find_symbol("PRICE_HEARTBEAT")
        .expect("the heartbeat's symbol");
    program
        .write_symbol(symbol, &Value::UDInt(count))
        .expect("the heartbeat");
}

/// Puts the `REAL` `value` at `address` in the process image that the next scan samples into the
/// variable located there: the tank's temperature at `%ID0`, as a write of the simulated input
/// does, and the service's setpoint at `%MD0`, as the runtime holds a service's write of a located
/// variable in its image (specification 44 §13.3). The image is untyped: the double word holds the
/// `REAL`'s bits.
fn put(program: &mut NativeProgram<'_, HostCode>, address: &str, value: f32) {
    let parsed = IoAddress::parse(address).expect("the address");
    program
        .write_address(&parsed, Value::DWord(value.to_bits()))
        .expect(address);
}

/// Scans every 100 ms from `*time` up to `until` ms, then gives `service_ok`, `target` and `heater`.
fn run_to(program: &mut NativeProgram<'_, HostCode>, time: &mut i64, until: i64) -> [Value; 3] {
    while *time <= until {
        program
            .scan(Duration::from_millis(*time), &mut NoHooks)
            .expect("the scan runs");
        *time += INTERVAL_MS;
    }
    ["HEATER.SERVICE_OK", "HEATER.TARGET", "HEATER.HEATER"]
        .map(|name| program.value(name).expect(name))
}

/// `service_ok`, `target` and `heater`.
fn heater(service_ok: bool, target: f32, heater: bool) -> [Value; 3] {
    [
        Value::Bool(service_ok),
        Value::Real(target),
        Value::Bool(heater),
    ]
}

#[test]
fn the_heater_takes_the_services_setpoint_only_while_its_heartbeat_is_fresh() {
    // The README's steps, worked by hand on the task's logical time: the task runs at its start and
    // then every 100 ms (specification 10 §4.3), and the tank reads 60 °C.
    // - Without the service, SERVICE_FRESH's first call, at 0 ms, sees heartbeat 0 without a change:
    //   not fresh, so the target is the safe 55.0, and the heater stays off, as 60 >= 55.
    // - The service writes 65.0, the setpoint of 38.2 EUR/MWh (below 50), and then its first
    //   heartbeat, which the runtime counts to 1, before the scan at 1000 ms: fresh, the target is
    //   65.0, and the heater turns on, as 60 < 65 - 2.
    // - Heartbeat 2 comes before the scan at 1500 ms, and heartbeat 3 with 50.0, the setpoint of
    //   120 EUR/MWh, before the scan at 2000 ms: the target is 50.0, and the heater turns off, as
    //   60 >= 50.
    // - The service stops after that heartbeat: at 4000 ms its age is exactly the 2 s timeout,
    //   still fresh; at 4100 ms it is not, and the target is the safe 55.0 again.
    // Catches an example that does not build as committed (a stale services.st, a `writes` path the
    // build refuses), a gate that takes the service's setpoint before its first heartbeat or after
    // the timeout, and a README whose values the program does not give.
    let root = example_copy("rust_service");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    put(&mut program, "%ID0", 60.0);
    let mut time = 0;
    assert_eq!(
        run_to(&mut program, &mut time, 0),
        heater(false, 55.0, false)
    );
    assert_eq!(
        run_to(&mut program, &mut time, 900),
        heater(false, 55.0, false)
    );

    put(&mut program, "%MD0", 65.0);
    beat(&mut program, 1);
    assert_eq!(
        run_to(&mut program, &mut time, 1000),
        heater(true, 65.0, true)
    );
    assert_eq!(
        run_to(&mut program, &mut time, 1400),
        heater(true, 65.0, true)
    );
    beat(&mut program, 2);
    assert_eq!(
        run_to(&mut program, &mut time, 1900),
        heater(true, 65.0, true)
    );

    put(&mut program, "%MD0", 50.0);
    beat(&mut program, 3);
    assert_eq!(
        run_to(&mut program, &mut time, 2000),
        heater(true, 50.0, false)
    );
    assert_eq!(
        run_to(&mut program, &mut time, 4000),
        heater(true, 50.0, false)
    );
    assert_eq!(
        run_to(&mut program, &mut time, 4100),
        heater(false, 55.0, false)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_services_committed_bindings_are_the_ones_its_program_gives() {
    // RU-62: `trust-dev service bindings price` writes the bindings from the built image, the
    // service's `writes` from trust.toml as `Writable` handles. The committed
    // price-service/src/plant.rs is that file, so the service the workspace compiles names the
    // program's variables with their types, and it can set Heater.price_setpoint alone. Catches a
    // change of the program without new bindings, which a user's service would meet as a refusal at
    // run time.
    let root = example_copy("rust_service");
    let Some((built, _)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let image = Image::parse(&built.image).expect("the image parses");
    let price = service_writes(&root)
        .expect("trust.toml reads")
        .into_iter()
        .find(|service| service.name == "price")
        .expect("the price service writes");
    assert_eq!(price.writes, ["Heater.price_setpoint"]);
    let generated = service_bindings(&image, "price", &price.writes).expect("the bindings");
    let committed = std::fs::read_to_string(root.join("price-service/src/plant.rs"))
        .expect("the committed bindings");
    assert_eq!(generated, committed);
    let _ = std::fs::remove_dir_all(root);
}
