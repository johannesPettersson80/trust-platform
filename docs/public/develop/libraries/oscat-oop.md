# OSCAT OOP

OSCAT OOP is the object-oriented facade over the classic
manual-aligned OSCAT package. It is useful when a project wants component
objects, interfaces, configuration methods, and read-only status properties
instead of direct OSCAT function-block calls everywhere.

Use [OSCAT](oscat.md) for the classic upstream-shaped surface. Use this page
for the OOP/component facade.

## Worked Example

The example [Cold Storage Alarm](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/OSCAT/cold_storage_alarm)
watches the chamber of the [OSCAT](oscat.md) manual's worked example: the
temperature in kelvin, and an alarm above −18 °C that clears only below −30 °C.
Its OOP program uses two components in place of the classic blocks:

- `UnitConverter.KelvinFromCelsius` returns the kelvin value of a temperature in
  degrees Celsius, 257.15 for −16 °C.
- `HysteresisSwitch` holds its limits as a `RealRange`, which `SetLimits` checks:
  a high limit below the low limit sets `Error` with the invalid-configuration
  error id and keeps the old limits. `Update` takes the measured value and
  returns the switch's state: TRUE above the high limit, FALSE below the low
  limit, unchanged in between.

The program makes the whole call sequence on every scan, in the order its tests
use: initialize, convert, set the limits, update. This is the program as the
example ships it:

```iecst
--8<-- "examples/OSCAT/cold_storage_alarm/oop/src/Main.st"
```

Its six test programs, in `oop/src/Tests.st`, check the warm chamber against the
classic `HYST`; readings from warm through the band to frozen; an alarm that holds
inside the band; the exact limits, which the switch must pass to change; two
chambers that do not share a state; and inverted limits, which set `Error`.
`trust-dev test --project examples/OSCAT/cold_storage_alarm/oop` runs them,
and so does CI. The example's README compares the two versions: the classic
program fits one chamber with constant limits, and the components earn their cost
when a second chamber needs the same conversion and deadband.

## Library Guide

--8<-- "docs/guides/OSCAT_OOP_LIBRARY_GUIDE.md:3"

## Related

- [OSCAT](oscat.md)
- [Libraries overview](index.md)
- [Structured Text naming standard](../st-naming-standard.md)
- [Libraries and Motion examples](../../examples/libraries-and-motion.md)
