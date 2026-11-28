//! A project's `io.toml` on a compiled program (specification 44 §8.8): the enabled drivers in their
//! order, each with the link it runs (specification 42, §2) and an MQTT driver's tag mappings
//! bound to the program's variables first (§13.3). The start builds the drivers of this plan, and
//! `trust-runtime validate`, which builds none, finds from it the composed drivers that would fill
//! the same input bytes (specification 11 §6.5).

use smol_str::SmolStr;

use super::CompiledRuntime;
use crate::config::IoConfig;
use crate::error::RuntimeError;
use crate::io::{InputOverlap, IoDriverRegistry};

/// One enabled driver of `io.toml` as the start adds it.
struct PlannedDriver {
    /// Its name in `io.toml`.
    name: SmolStr,
    /// The link it runs (specification 42, §2).
    link: Option<String>,
    /// Its parameters, an MQTT driver's tag mappings resolved to points (specification 32 §5).
    params: toml::Value,
}

impl CompiledRuntime {
    /// Takes `io`, a project's `io.toml`, as trust-runtime starts a compiled program
    /// (specification 44 §8.8): its safe outputs, and each enabled driver after the others, an MQTT
    /// driver with its tag mappings bound to the program's variables first (§13.3).
    ///
    /// # Errors
    ///
    /// The error of a tag mapping or a driver that does not build.
    pub fn add_io_config(&mut self, io: &IoConfig) -> Result<(), RuntimeError> {
        self.set_io_safe_state(io.safe_state.clone());
        let registry = IoDriverRegistry::default_registry();
        for driver in self.plan_io_drivers(io)? {
            if let Some(spec) = registry.build(driver.name.as_str(), &driver.params)? {
                match driver.link {
                    Some(link) => self.add_io_driver_with_link(spec.name, link, spec.driver),
                    None => self.add_io_driver(spec.name, spec.driver),
                }
            }
        }
        Ok(())
    }

    /// The composed drivers of `io` that would fill the same bytes of the input image, from their
    /// parameters alone, as `trust-runtime validate` reports them (specification 11 §6.5): each
    /// enabled driver's declared bytes (specification 42, §5 item 4), in their order, within the
    /// image the drivers would see, the program's own with the bytes that the MQTT tags bind after
    /// it. It builds no driver.
    ///
    /// # Errors
    ///
    /// The error of a tag mapping, a driver the registry does not know or parameters it refuses.
    pub fn io_config_input_overlaps(
        &mut self,
        io: &IoConfig,
    ) -> Result<Vec<InputOverlap>, RuntimeError> {
        let planned = self.plan_io_drivers(io)?;
        let registry = IoDriverRegistry::default_registry();
        let outputs = self.driver_output_len();
        let declared = planned
            .iter()
            .map(|driver| {
                let name = driver
                    .link
                    .as_deref()
                    .map_or_else(|| driver.name.clone(), SmolStr::new);
                registry
                    .declared_inputs(driver.name.as_str(), &driver.params, outputs)
                    .map(|bytes| (name, bytes))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crate::io::input_overlaps(
            &declared,
            self.driver_input_len(),
        ))
    }

    /// The enabled drivers of `io` in their order, as the start adds them. Each runs the link
    /// Devices & Connections names it by (specification 42, §2), and a driver switched off in
    /// `io.toml` reads Disabled (§3 item 1). An MQTT driver's tag mappings are resolved against
    /// the program and the bytes past its image that they bind are bound now, so that the image
    /// the drivers see has its final size (§13.3).
    fn plan_io_drivers(&mut self, io: &IoConfig) -> Result<Vec<PlannedDriver>, RuntimeError> {
        let mut planned = Vec::new();
        for (position, driver) in io.drivers.iter().enumerate() {
            let link = crate::io::io_link_name(driver.name.as_str(), position);
            if !driver.enabled {
                if let Some(link) = link {
                    self.add_disabled_link(link);
                }
                continue;
            }
            let mut params = driver.params.clone();
            if crate::io::takes_tag_mappings(driver.name.as_str()) {
                let (lowered, links) =
                    crate::io::resolve_compiled_mqtt_tag_mappings(self, &driver.params)?;
                self.link_mqtt_tags(&links)?;
                params = lowered;
            }
            planned.push(PlannedDriver {
                name: driver.name.clone(),
                link,
                params,
            });
        }
        Ok(planned)
    }
}
