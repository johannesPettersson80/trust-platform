# OSCAT Examples

This folder is the OSCAT example catalog. Every example is a paired comparison:
the same process is implemented once in classic procedural Structured Text and
once with the OSCAT OOP object-oriented style.

## Folder Contract

Each example folder has this shape:

```text
examples/OSCAT/<example>/
  README.md      # process and OOP-pattern explanation for the pair
  non-oop/       # classic/procedural ST project
  oop/           # OSCAT OOP ST project
```

The README lives next to the two projects because it explains the comparison,
not just one implementation. Open the README first, then read `non-oop/src/Main.st`
as the baseline and `oop/src/Main.st` as the pattern implementation.

## How This Teaches OOP

The catalog is process-first. Each pair starts with a real industrial problem:
recipe selection, mode switching, lead/lag arbitration, alarm fan-out, vendor
adapter boundaries, state-specific behavior, command auditability, or signal
conditioning.

The non-OOP project shows the direct branch/state-machine implementation. The
OOP project shows the extension point as a named interface, function block,
state object, command object, adapter, facade, mediator, decorator, observer,
builder, proxy, or composed component. The README explains what the pattern is,
why it fits that process, how to reuse it, and when classic ST is still better.

## Run A Pair

```bash
trust-runtime test --project examples/OSCAT/airport_baggage_command_observer/non-oop
trust-runtime test --project examples/OSCAT/airport_baggage_command_observer/oop
```

## Catalog

| Example | Pattern | Category | Projects |
| --- | --- | --- | --- |
| [Airport Baggage Command Observer](./airport_baggage_command_observer/README.md) | Command + Observer | industrial OOP pattern example | [non-oop](./airport_baggage_command_observer/non-oop/) / [oop](./airport_baggage_command_observer/oop/) |
| [Battery Energy Storage Facade](./battery_energy_storage_facade/README.md) | Facade + Observer | industrial OOP pattern example | [non-oop](./battery_energy_storage_facade/non-oop/) / [oop](./battery_energy_storage_facade/oop/) |
| [Boiler Feedwater Alarm](./boiler_feedwater_alarm/README.md) | Component Composition | compact component-composition showcase | [non-oop](./boiler_feedwater_alarm/non-oop/) / [oop](./boiler_feedwater_alarm/oop/) |
| [Boiler Room Heating Plant](./boiler_room_heating_plant/README.md) | Facade + Observer | industrial OOP pattern example | [non-oop](./boiler_room_heating_plant/non-oop/) / [oop](./boiler_room_heating_plant/oop/) |
| [Booster Commissioning Decorator](./booster_commissioning_decorator/README.md) | Decorator | industrial OOP pattern example | [non-oop](./booster_commissioning_decorator/non-oop/) / [oop](./booster_commissioning_decorator/oop/) |
| [Chemical Dosing Command](./chemical_dosing_command/README.md) | Command + Memento | industrial OOP pattern example | [non-oop](./chemical_dosing_command/non-oop/) / [oop](./chemical_dosing_command/oop/) |
| [Chiller Temperature PID](./chiller_temperature_pid/README.md) | Component Composition | compact component-composition showcase | [non-oop](./chiller_temperature_pid/non-oop/) / [oop](./chiller_temperature_pid/oop/) |
| [CIP Wash State](./cip_wash_state/README.md) | State | industrial OOP pattern example | [non-oop](./cip_wash_state/non-oop/) / [oop](./cip_wash_state/oop/) |
| [Cleanroom Pressure Strategy Composite](./cleanroom_pressure_strategy_composite/README.md) | Strategy + Composite | industrial OOP pattern example | [non-oop](./cleanroom_pressure_strategy_composite/non-oop/) / [oop](./cleanroom_pressure_strategy_composite/oop/) |
| [Closed Loop Polymorphism](./closed_loop_polymorphism/README.md) | Interface Polymorphism | compact OOP pattern showcase | [non-oop](./closed_loop_polymorphism/non-oop/) / [oop](./closed_loop_polymorphism/oop/) |
| [Cold Storage Alarm](./cold_storage_alarm/README.md) | Component Composition | compact component-composition showcase | [non-oop](./cold_storage_alarm/non-oop/) / [oop](./cold_storage_alarm/oop/) |
| [Cold Storage Plant](./cold_storage_plant/README.md) | Composite + Observer + Mediator | industrial OOP pattern example | [non-oop](./cold_storage_plant/non-oop/) / [oop](./cold_storage_plant/oop/) |
| [Compressor Pressure Filter](./compressor_pressure_filter/README.md) | Component Composition | compact component-composition showcase | [non-oop](./compressor_pressure_filter/non-oop/) / [oop](./compressor_pressure_filter/oop/) |
| [Conveyor Pulse](./conveyor_pulse/README.md) | Component Composition | compact component-composition showcase | [non-oop](./conveyor_pulse/non-oop/) / [oop](./conveyor_pulse/oop/) |
| [Cooling Tower Facade Template](./cooling_tower_facade_template/README.md) | Facade + Template Method | industrial OOP pattern example | [non-oop](./cooling_tower_facade_template/non-oop/) / [oop](./cooling_tower_facade_template/oop/) |
| [Crane Hoist Adapter State](./crane_hoist_adapter_state/README.md) | Adapter + State | industrial OOP pattern example | [non-oop](./crane_hoist_adapter_state/non-oop/) / [oop](./crane_hoist_adapter_state/oop/) |
| [Dairy Separator Adapter State](./dairy_separator_adapter_state/README.md) | Adapter + State | industrial OOP pattern example | [non-oop](./dairy_separator_adapter_state/non-oop/) / [oop](./dairy_separator_adapter_state/oop/) |
| [District Pump Network Proxy Mediator](./district_pump_network_proxy_mediator/README.md) | Proxy + Mediator | industrial OOP pattern example | [non-oop](./district_pump_network_proxy_mediator/non-oop/) / [oop](./district_pump_network_proxy_mediator/oop/) |
| [Energy Normalization](./energy_normalization/README.md) | Component Composition | compact component-composition showcase | [non-oop](./energy_normalization/non-oop/) / [oop](./energy_normalization/oop/) |
| [Filter Backwash Template](./filter_backwash_template/README.md) | Template Method | industrial OOP pattern example | [non-oop](./filter_backwash_template/non-oop/) / [oop](./filter_backwash_template/oop/) |
| [Greenhouse Temperature](./greenhouse_temperature/README.md) | Component Composition | compact component-composition showcase | [non-oop](./greenhouse_temperature/non-oop/) / [oop](./greenhouse_temperature/oop/) |
| [HVAC Air Handling Unit](./hvac_air_handling_unit/README.md) | Strategy | industrial OOP pattern example | [non-oop](./hvac_air_handling_unit/non-oop/) / [oop](./hvac_air_handling_unit/oop/) |
| [Irrigation Sun Clock](./irrigation_sun_clock/README.md) | Component Composition | compact component-composition showcase | [non-oop](./irrigation_sun_clock/non-oop/) / [oop](./irrigation_sun_clock/oop/) |
| [Kiln Dryer Decorator Strategy](./kiln_dryer_decorator_strategy/README.md) | Decorator + Strategy | industrial OOP pattern example | [non-oop](./kiln_dryer_decorator_strategy/non-oop/) / [oop](./kiln_dryer_decorator_strategy/oop/) |
| [Maintenance Stack](./maintenance_stack/README.md) | Component Composition | compact component-composition showcase | [non-oop](./maintenance_stack/non-oop/) / [oop](./maintenance_stack/oop/) |
| [Mixed Vendor VFD Adapter](./mixed_vendor_vfd_adapter/README.md) | Adapter | industrial OOP pattern example | [non-oop](./mixed_vendor_vfd_adapter/non-oop/) / [oop](./mixed_vendor_vfd_adapter/oop/) |
| [Multi Product Batch Reactor](./multi_product_batch_reactor/README.md) | Factory + Template Method | industrial OOP pattern example | [non-oop](./multi_product_batch_reactor/non-oop/) / [oop](./multi_product_batch_reactor/oop/) |
| [Packaging Reject Pulse](./packaging_reject_pulse/README.md) | Component Composition | compact component-composition showcase | [non-oop](./packaging_reject_pulse/non-oop/) / [oop](./packaging_reject_pulse/oop/) |
| [Pasteurizer Quality Chain](./pasteurizer_quality_chain/README.md) | Chain of Responsibility + Template Method | industrial OOP pattern example | [non-oop](./pasteurizer_quality_chain/non-oop/) / [oop](./pasteurizer_quality_chain/oop/) |
| [Pharma Filling Builder State](./pharma_filling_builder_state/README.md) | Builder + State | industrial OOP pattern example | [non-oop](./pharma_filling_builder_state/non-oop/) / [oop](./pharma_filling_builder_state/oop/) |
| [Production Queue](./production_queue/README.md) | Component Composition | compact component-composition showcase | [non-oop](./production_queue/non-oop/) / [oop](./production_queue/oop/) |
| [Pump Pressure](./pump_pressure/README.md) | Component Composition | compact component-composition showcase | [non-oop](./pump_pressure/non-oop/) / [oop](./pump_pressure/oop/) |
| [Recipe Batch Stack](./recipe_batch_stack/README.md) | Component Composition | compact component-composition showcase | [non-oop](./recipe_batch_stack/non-oop/) / [oop](./recipe_batch_stack/oop/) |
| [Refinery Temperature Conditioning](./refinery_temperature_conditioning/README.md) | Decorator | industrial OOP pattern example | [non-oop](./refinery_temperature_conditioning/non-oop/) / [oop](./refinery_temperature_conditioning/oop/) |
| [Robotic Palletizer Command State](./robotic_palletizer_command_state/README.md) | Command + State | industrial OOP pattern example | [non-oop](./robotic_palletizer_command_state/non-oop/) / [oop](./robotic_palletizer_command_state/oop/) |
| [Shift Order Queue](./shift_order_queue/README.md) | Component Composition | compact component-composition showcase | [non-oop](./shift_order_queue/non-oop/) / [oop](./shift_order_queue/oop/) |
| [Silo Loading Composite Mediator](./silo_loading_composite_mediator/README.md) | Composite + Mediator | industrial OOP pattern example | [non-oop](./silo_loading_composite_mediator/non-oop/) / [oop](./silo_loading_composite_mediator/oop/) |
| [Solar Lighting Clock](./solar_lighting_clock/README.md) | Component Composition | compact component-composition showcase | [non-oop](./solar_lighting_clock/non-oop/) / [oop](./solar_lighting_clock/oop/) |
| [Tank Farm Transfer Skid](./tank_farm_transfer_skid/README.md) | Composite + Iterator | industrial OOP pattern example | [non-oop](./tank_farm_transfer_skid/non-oop/) / [oop](./tank_farm_transfer_skid/oop/) |
| [Tank Level PID](./tank_level_pid/README.md) | Component Composition | compact component-composition showcase | [non-oop](./tank_level_pid/non-oop/) / [oop](./tank_level_pid/oop/) |
| [Temperature Zone Composition](./temperature_zone_composition/README.md) | Composition | compact OOP pattern showcase | [non-oop](./temperature_zone_composition/non-oop/) / [oop](./temperature_zone_composition/oop/) |
| [Tunnel Oven Strategy Observer](./tunnel_oven_strategy_observer/README.md) | Strategy + Observer | industrial OOP pattern example | [non-oop](./tunnel_oven_strategy_observer/non-oop/) / [oop](./tunnel_oven_strategy_observer/oop/) |
| [Tunnel Washer Chain](./tunnel_washer_chain/README.md) | Chain of Responsibility | industrial OOP pattern example | [non-oop](./tunnel_washer_chain/non-oop/) / [oop](./tunnel_washer_chain/oop/) |
| [Ventilation Filter](./ventilation_filter/README.md) | Component Composition | compact component-composition showcase | [non-oop](./ventilation_filter/non-oop/) / [oop](./ventilation_filter/oop/) |
| [Warehouse Conveyor Merge Mediator](./warehouse_conveyor_merge_mediator/README.md) | Mediator | industrial OOP pattern example | [non-oop](./warehouse_conveyor_merge_mediator/non-oop/) / [oop](./warehouse_conveyor_merge_mediator/oop/) |
| [Wastewater Aeration](./wastewater_aeration/README.md) | Component Composition | compact component-composition showcase | [non-oop](./wastewater_aeration/non-oop/) / [oop](./wastewater_aeration/oop/) |
| [Water Booster Pump Station](./water_booster_pump_station/README.md) | Mediator + Observer | industrial OOP pattern example | [non-oop](./water_booster_pump_station/non-oop/) / [oop](./water_booster_pump_station/oop/) |
| [Weather Station Conversion](./weather_station_conversion/README.md) | Component Composition | compact component-composition showcase | [non-oop](./weather_station_conversion/non-oop/) / [oop](./weather_station_conversion/oop/) |
| [Wind Speed Alarm](./wind_speed_alarm/README.md) | Component Composition | compact component-composition showcase | [non-oop](./wind_speed_alarm/non-oop/) / [oop](./wind_speed_alarm/oop/) |

## Acceptance Rule

An example may only claim a pattern, communication boundary, alarm/event record,
or historian/MQTT/OPC UA integration when the project files and README name the
actual ST structure or runtime binding that backs the claim.
