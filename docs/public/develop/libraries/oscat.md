# OSCAT

OSCAT support covers the imported surface, useful function families, and the
compatibility boundary.

Use [Libraries](index.md) for package/project ownership. Use [Project Layout](../project-layout.md)
for project structure.

Use [OSCAT OOP](oscat-oop.md) when the same OSCAT-backed
application logic should be written with objects, narrow interfaces, and
Structured Text component tests.

## Worked Example

The example [Cold Storage Alarm](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/OSCAT/cold_storage_alarm)
watches a cold-storage chamber. It reports the chamber's temperature in kelvin,
and raises an alarm above −18 °C that clears only below −30 °C, so that the alarm
does not chatter while the chamber defrosts. Its classic program calls two OSCAT
blocks, each instance once per scan:

- `TEMPERATURE` converts between temperature scales. Each input defaults to
  absolute zero in its own scale, so a call that sets only `C` converts degrees
  Celsius: `YK` is the kelvin value, 257.15 for −16 °C.
- `HYST` switches with a deadband. `Q` goes TRUE when `In` rises above
  `HIGH_LIMIT` and FALSE when it falls below `LOW_LIMIT`, and keeps its value in
  between, where `WIN` is TRUE.

This is the program as the example ships it:

```iecst
--8<-- "examples/OSCAT/cold_storage_alarm/non-oop/src/Main.st"
```

Its test program `ColdStorageAlarmClassicDetectsWarmChamber`, in
`non-oop/src/Tests.st`, calls both blocks at −16 °C and expects 257.15 K and the
alarm on; `trust-dev test --project examples/OSCAT/cold_storage_alarm/non-oop`
runs it, and so does CI. The same chamber written with the OSCAT OOP components
is the worked example of [OSCAT OOP](oscat-oop.md).

## Library Guide

--8<-- "docs/guides/OSCAT_LIBRARY_GUIDE.md:3"

## Related

- [Libraries overview](index.md)
- [OSCAT OOP](oscat-oop.md)
- [Project Layout](../project-layout.md)
- [Libraries and Motion examples](../../examples/libraries-and-motion.md)
