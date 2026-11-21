//! The price service of the example `rust_service`. Every second it fetches the electricity price
//! over HTTP and writes the heater's setpoint for it into the program that runs in the runtime. It
//! sends its heartbeat only while it has a current price, so that the program falls back to its
//! safe setpoint when the service or its price source stops.

#![forbid(unsafe_code)]

// The bindings `trust-dev service bindings price` writes; rustfmt leaves them as written.
#[rustfmt::skip]
mod plant;
mod price;

use std::time::Duration;

use trust_service::{Heartbeat, Service, ServiceError};

/// The price source unless `PRICE_SOURCE` names another: the stand-in of
/// `cargo run --example stand_in_source`.
const STAND_IN: &str = "http://127.0.0.1:47801/price";

/// How often the service fetches the price.
const EVERY: Duration = Duration::from_secs(1);

fn main() -> Result<(), ServiceError> {
    let source = std::env::var("PRICE_SOURCE").unwrap_or_else(|_| STAND_IN.to_owned());
    // [services.price] of the project's trust.toml: the token and the heartbeat, and the endpoint
    // that the project's running runtime published.
    let service = Service::connect("price")?;
    let mut heartbeat: Option<Heartbeat> = None;
    let mut told = false;
    loop {
        match price::fetch(&source) {
            Ok(price) => {
                let setpoint = price::setpoint_for(price);
                // The setpoint first: the program never takes an old one as fresh.
                service.set(plant::Heater::price_setpoint, &setpoint)?;
                if heartbeat.is_none() {
                    heartbeat = Some(service.start_heartbeat()?);
                }
                println!("{price:.1} EUR/MWh: setpoint {setpoint:.1} °C");
                told = false;
            }
            Err(error) => {
                // Dropping the heartbeat stops it.
                if heartbeat.take().is_some() || !told {
                    println!(
                        "No price from {source}: {error}. No heartbeat: the heater uses its safe setpoint."
                    );
                    told = true;
                }
            }
        }
        std::thread::sleep(EVERY);
    }
}
