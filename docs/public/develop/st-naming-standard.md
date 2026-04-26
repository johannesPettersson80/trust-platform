# Structured Text Naming Standard

New truST-authored public APIs use readable PascalCase names. Imported or
standardized public names keep their upstream spelling.

This gives new code a consistent house style without breaking PLCopen, OSCAT,
PROFINET, vendor-profile, or IEC compatibility.

## Governing Rule

- New truST-owned public names: PascalCase domain names.
- Inherited public symbols: preserve upstream spelling exactly.
- Compatibility beats cosmetic consistency for imported public contracts.

Examples:

- New truST names: `PidController`, `TargetValue`, `SetLimits`,
  `DefaultPidKp`.
- Preserved names: `CTRL_PID`, `FT_PT1`, `MC_MoveAbsolute`,
  `mcERR_NotSupported`, `PN_SWLimitPos`, `STRING_LENGTH`.

## Public API Names

| Kind | Standard | Example |
| --- | --- | --- |
| Function block, class, struct, enum, union | PascalCase noun | `PidController`, `RealRange` |
| Interface | `I` + PascalCase noun | `IPidController` |
| Method | PascalCase verb phrase | `SetLimits`, `Update`, `CalculateSunTime` |
| Property | PascalCase noun phrase | `Output`, `TargetValue`, `Ready` |
| Function | PascalCase for truST-owned, upstream spelling for imported | `BuildReport`, `DEG_TO_DIR` |
| Enum literal | Domain-prefixed PascalCase | `ComponentStateReady`, `QueueResultFull` |
| Constants | PascalCase noun phrase | `DefaultPidKp`, `MaxDwordFifo16Capacity` |
| Source file | PascalCase primary POU name for one-primary-POU files; lower_snake domain name for multi-POU package files | `PidController.st`, `component_interfaces.st` |

## Variables And Parameters

- `VAR_INPUT`, `VAR_OUTPUT`, and `VAR_IN_OUT` names use PascalCase semantic
  names: `Target`, `MeasuredValue`, `MaxLimit`.
- Method parameters use PascalCase semantic names: `SetLimits(MinValue,
  MaxValue)` or `SetLimits(Limits)`.
- Internal `VAR` names use PascalCase semantic names: `ErrorValue`,
  `LastSample`, `IntegralAccumulator`.
- Short loop variables may use single uppercase letters: `I`, `J`, `K`.
- Longer-scoped loop/index variables use PascalCase names: `RowIndex`,
  `ChannelIndex`.
- Multi-instance names should describe the domain role:
  `PidLevel`, `PidPressure`, `IntakeFilter`, not `Pid1`, `Pid2`.

## Constants

Constants use PascalCase and describe the value, not its storage class.

```st
VAR CONSTANT
    DefaultPidKp : REAL := REAL#1.0;
    MaxDwordFifo16Capacity : UINT := UINT#16;
END_VAR
```

Placement rule:

- Keep constants in the consuming POU when they are private to that POU.
- Move constants to package/file scope only when multiple POUs share them.
- Do not add Hungarian prefixes such as `c_`, `r`, `i`, or `b` to new
  truST-owned constants.

## Methods And Properties

- Properties are read-only snapshots unless the domain has a strong reason for
  writeable properties.
- Methods change state or perform a command.
- Scan-cycle objects use one scan method consistently, normally `Update(...)`.
- Setter methods return no value unless failure is meaningful.
- Methods that return another object or result record must make ownership and
  lifetime obvious in the name and documentation.

## Internal Implementation Names

Public names follow the PascalCase standard. Internal hidden globals or
generated implementation details may use lower snake case with a `g_` prefix
when the prefix marks non-public global state.

Examples:

- Public: `DefaultPidKp`
- Internal hidden global: `g_component_constants_loaded`

Do not mix styles within a file except for preserved inherited symbols.

## Exceptions

Preserve inherited names when changing them would break recognition or
compatibility:

- IEC names and literals.
- PLCopen Motion names and enum literals.
- OSCAT function, function-block, type, and global names in the classic OSCAT
  package.
- PROFINET/fieldbus profile symbols.
- Vendor-imported examples and compatibility fixtures.

When an exception is intentional, document it in the relevant library guide,
example README, or audit checklist.
