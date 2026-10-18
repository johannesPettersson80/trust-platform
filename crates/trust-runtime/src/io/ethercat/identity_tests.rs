use std::cell::{Cell, RefCell};

use smol_str::SmolStr;

use super::*;

/// A bus that answers an identification with canned SubDevices and records no exchange.
struct FakeBus {
    discovery: EthercatDiscovery,
}

impl EthercatBus for FakeBus {
    fn discover(&mut self, _config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        Ok(self.discovery.clone())
    }

    fn read_inputs(&mut self, _bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        Err(RuntimeError::IoDriver("fake bus reads nothing".into()))
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Err(RuntimeError::IoDriver("fake bus writes nothing".into()))
    }
}

#[test]
fn ethercat_identification_stays_below_op() {
    // Spec 11, "Safety": "EtherCAT identification maps the process data in PRE-OP and never brings
    // the bus to SAFE-OP or OP". The fake opener records the state each bus was asked for; a
    // PRE-OP-only identification asks for `PreOpPdi` and never for `Op`. The wrong behaviour it
    // catches: identification opening the bus in OP, which makes outputs live.
    let config = EthercatConfig::from_params(
        &toml::from_str("adapter = 'eth-identity-test'").expect("params"),
    )
    .expect("config");
    let discovery = EthercatDiscovery {
        subdevices: vec![EthercatSubDevice {
            slot: 0,
            configured_address: 1001,
            alias_address: 0,
            name: SmolStr::new("EK1100"),
            channels: 1,
            vendor_id: 0x0000_0002,
            product_id: 0x044c_2a22,
            revision: 0x0011_0000,
            serial: 0,
            input_bytes: 0,
            output_bytes: 0,
        }],
        input_bytes: 0,
        output_bytes: 0,
    };
    let states = RefCell::new(Vec::new());
    let open = |_config: &EthercatConfig,
                state: EthercatBusState|
     -> Result<Box<dyn EthercatBus>, RuntimeError> {
        states.borrow_mut().push(state);
        Ok(Box::new(FakeBus {
            discovery: discovery.clone(),
        }))
    };

    let found = identify_ethercat_subdevices(&config, open).expect("identification");

    assert_eq!(
        *states.borrow(),
        [EthercatBusState::PreOpPdi],
        "identification must map the process data in PRE-OP and never reach SAFE-OP or OP"
    );
    assert_eq!(found.subdevices.len(), 1);
}

#[test]
fn opening_an_adapter_twice_reuses_its_pdu_storage() {
    // O22: one main-device storage per adapter, reused by every later opening. The first storage is
    // leaked and stays alive, so a second allocation cannot take its address; equal addresses
    // therefore prove that the second opening reused the first storage instead of leaking another.
    let first = adapter_pdu_storage("eth-identity-test");
    let second = adapter_pdu_storage("eth-identity-test");
    assert!(
        std::ptr::eq(first, second),
        "opening an adapter twice must reuse its PDU storage"
    );
}

/// A bus whose scans grow: the first scan reports one SubDevice, later scans two, so a cached bus
/// shows a device added after the first scan while a fresh bus would look like a first scan again.
struct GrowingBus {
    scans: usize,
}

impl EthercatBus for GrowingBus {
    fn discover(&mut self, _config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        self.scans += 1;
        let count: u16 = if self.scans == 1 { 1 } else { 2 };
        let subdevices = (0..count)
            .map(|slot| EthercatSubDevice {
                slot,
                configured_address: 1000 + slot,
                alias_address: 0,
                name: SmolStr::new("EK1100"),
                channels: 1,
                vendor_id: 0x0000_0002,
                product_id: 0x044c_2a22,
                revision: 0x0011_0000,
                serial: 0,
                input_bytes: 0,
                output_bytes: 0,
            })
            .collect();
        Ok(EthercatDiscovery {
            subdevices,
            input_bytes: 0,
            output_bytes: 0,
        })
    }

    fn read_inputs(&mut self, _bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        Err(RuntimeError::IoDriver("growing bus reads nothing".into()))
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Err(RuntimeError::IoDriver("growing bus writes nothing".into()))
    }
}

#[test]
fn a_device_added_after_the_first_scan_is_found_by_the_next() {
    // O22 and spec 11, "Sources": an identification reuses the adapter's main device and
    // re-enumerates the bus on every scan, so a device added after the first scan is found by the
    // next. The wrong behaviour it catches: caching the first scan's group instead of the main
    // device, which keeps reporting the stale bus and never shows a device plugged in later.
    let config = EthercatConfig::from_params(
        &toml::from_str("adapter = 'eth-scan-growth-test'").expect("params"),
    )
    .expect("config");
    let opens = Cell::new(0usize);
    let open_bus = |_config: &EthercatConfig| -> Result<Box<dyn EthercatBus>, RuntimeError> {
        opens.set(opens.get() + 1);
        Ok(Box::new(GrowingBus { scans: 0 }))
    };
    let first = cached_identification_bus(&config, open_bus).expect("first opening");
    let first_scan = first
        .lock()
        .expect("first bus lock")
        .discover(&config)
        .expect("first scan");

    let open_bus = |_config: &EthercatConfig| -> Result<Box<dyn EthercatBus>, RuntimeError> {
        opens.set(opens.get() + 1);
        Ok(Box::new(GrowingBus { scans: 0 }))
    };
    let second = cached_identification_bus(&config, open_bus).expect("second opening");
    let second_scan = second
        .lock()
        .expect("second bus lock")
        .discover(&config)
        .expect("second scan");

    assert_eq!(
        first_scan.subdevices.len(),
        1,
        "the first scan reports the one device present then"
    );
    assert_eq!(
        second_scan.subdevices.len(),
        2,
        "the next scan reports the device added since"
    );
    assert_eq!(
        opens.get(),
        1,
        "the adapter's bus, and with it its PDU storage, is opened once for both scans"
    );
}
